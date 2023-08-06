# coding=utf-8
# Copyright 2023 Michael Feil.
#
# This code is loosely based on Huggingface text-generation-inference v0.9.3's causal_lm.py implementation.
# While it remains licensed under Apache License, Version 2.0,
# text-generation-inference itself on 7/28/2023 has changed its license.
# This code remains unaffected by this change.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import torch
import numpy as np
import os
import multiprocessing
from pathlib import Path

from huggingface_hub.constants import HUGGINGFACE_HUB_CACHE
from opentelemetry import trace
from transformers import (
    AutoTokenizer,
    AutoConfig,
)
from typing import Optional, Tuple, List, Type, Dict

from text_generation_server.models import Model
from text_generation_server.models.types import (
    PrefillTokens,
    Generation,
    GeneratedText,
)

from text_generation_server.utils import Sampling
from text_generation_server.models.causal_lm import CausalLMBatch

try:
    import ctranslate2
except ImportError:
    ctranslate2 = None


tracer = trace.get_tracer(__name__)


class CT2CausalLM(Model):
    def __init__(
        self,
        model_id: str,
        revision: Optional[str] = None,
        quantize: Optional[str] = None,
        dtype: Optional[torch.dtype] = None,
        trust_remote_code: bool = False,
    ):
        if ctranslate2 is None:
            raise ValueError(
                "for quantization with ct2, the installation requires the pip package ctranslate2. "
                "install via `text-generation-server[ct2]` or `pip install ctranslate2` is required.",
            )

        tokenizer = AutoTokenizer.from_pretrained(
            model_id,
            revision=revision,
            padding_side="left",
            truncation_side="left",
            trust_remote_code=trust_remote_code,
        )

        # Start CT2
        ct2_generator_kwargs = {
            "inter_threads": int(os.environ.get("TGI_CT2_INTER_THREADS", 1))
        }
        if torch.cuda.is_available():
            self.ct2_device = "cuda"
            ct2_generator_kwargs["intra_threads"] = int(
                os.environ.get("TGI_CT2_INTRA_THREADS", 1)
            )
        else:
            self.ct2_device = "cpu"
            ct2_generator_kwargs["intra_threads"] = int(
                os.environ.get(
                    "TGI_CT2_INTRA_THREADS", multiprocessing.cpu_count() // 2
                )
            )

        if dtype == torch.float16 and self.ct2_device == "cuda":
            ct2_compute_type = "float16"
        elif dtype == torch.bfloat16 and self.ct2_device == "cuda":
            ct2_compute_type = "bfloat16"
        elif self.ct2_device == "cpu" and dtype in [torch.float16, torch.bfloat16]:
            # float16 is not available on CPU
            # and int16 has no stable implementation
            ct2_compute_type = "float32"
        else:
            # default, int8 quantization.

            if "cuda" in self.ct2_device:
                # int8 for int8 layers, float16 for non-quantized layers
                ct2_compute_type = "int8_float16"
            else:
                # int8 for int8 layers, float32 for non-quantized layers
                ct2_compute_type = "int8"

        # Start CT2 - conversion
        out_dir = (
            Path(HUGGINGFACE_HUB_CACHE)
            / "ct2models" / f"{model_id.replace('/','--')}--{ct2_compute_type}"
        )

        if not os.path.exists(out_dir / "model.bin"):
            try:
                converter = ctranslate2.converters.TransformersConverter(
                    model_id,
                    activation_scales=None,
                    load_as_float16=ct2_compute_type != "bfloat16",
                    revision=revision,
                    low_cpu_mem_usage=True,
                    trust_remote_code=trust_remote_code,
                )
                converter.convert(
                    output_dir=out_dir,
                    vmap=None,
                    quantization=ct2_compute_type,
                    force=True,
                )
            except Exception as ex:
                raise ValueError(
                    f"conversion with ctranslate2 for {model_id} failed : Error {ex}"
                )
        if not os.path.exists(out_dir / "model.bin"):
            raise ValueError(
                f"no ctranslate2 model for {model_id} found after conversion in {out_dir}"
            )

        # Start CT2
        self.ct2_model = ctranslate2.Generator(
            str(out_dir),
            device=self.ct2_device,
            compute_type=ct2_compute_type,
            **ct2_generator_kwargs,
        )

        class DummyModel(torch.nn.Module):
            def __init__(self, *args, **kwargs) -> None:
                super().__init__(*args, **kwargs)
                self.config = AutoConfig.from_pretrained(
                    model_id, revision=revision, trust_remote_code=trust_remote_code
                )

        model = DummyModel()

        if tokenizer.pad_token_id is None:
            if model.config.pad_token_id is not None:
                tokenizer.pad_token_id = model.config.pad_token_id
            elif model.config.eos_token_id is not None:
                tokenizer.pad_token_id = model.config.eos_token_id
            elif tokenizer.eos_token_id is not None:
                tokenizer.pad_token_id = tokenizer.eos_token_id
            else:
                tokenizer.add_special_tokens({"pad_token": "[PAD]"})

        super().__init__(
            model=model,
            tokenizer=tokenizer,
            requires_padding=True,
            dtype=torch.int8 if "int8" in ct2_compute_type else torch.float16,
            device=torch.device(self.ct2_device),
        )

    @property
    def batch_type(self) -> Type[CausalLMBatch]:
        return CausalLMBatch

    def decode(self, generated_ids: List[int]) -> str:
        return self.tokenizer.decode(
            generated_ids, skip_special_tokens=True, clean_up_tokenization_spaces=False
        )

    def forward_ct2(
        self,
        all_input_ids,
        input_lengths,
    ) -> Tuple[torch.Tensor, List[Tuple[torch.Tensor, torch.Tensor]]]:
        # CT2 forward requires a list of list of input tokens ids and lengths
        ids_input = (
            torch.nested.to_padded_tensor(
                torch.nested.nested_tensor(all_input_ids), 1234567
            )
            .flatten(1)
            .to(torch.int32)
        )
        # lengths of the padded ids_input, i.e. how often not pad=1234567 is used.
        lengths = np.array(input_lengths, dtype=np.int32)

        if self.ct2_device == "cuda":
            lengths = torch.from_numpy(lengths).to(self.ct2_device)
        elif self.ct2_device == "cpu":
            ids_input = ids_input.numpy()

        ids_input = ctranslate2.StorageView.from_array(ids_input)
        lengths = ctranslate2.StorageView.from_array(lengths)
        # now, forward through the network
        logits = self.ct2_model.forward_batch(ids_input, lengths)

        # continue with logits as torch tensor
        if self.ct2_device == "cpu":
            # logits is a float32 torch cpu tensor
            logits = torch.from_numpy(np.asarray(logits))
        else:
            # logits is a float16 torch cuda tensor
            logits = torch.as_tensor(logits, device=self.ct2_device)
        return logits, None

    @tracer.start_as_current_span("generate_token")
    def generate_token(
        self, batch: CausalLMBatch
    ) -> Tuple[List[Generation], Optional[CausalLMBatch]]:
        logits, past = self.forward_ct2(batch.all_input_ids, batch.input_lengths)

        # Results
        generations: List[Generation] = []
        stopped = True

        # Zipped iterator
        iterator = zip(
            batch.requests,
            batch.input_lengths,
            batch.prefix_offsets,
            batch.read_offsets,
            logits,
            batch.next_token_choosers,
            batch.stopping_criterias,
            batch.all_input_ids,
        )

        # For each member of the batch
        for i, (
            request,
            input_length,
            prefix_offset,
            read_offset,
            logits,
            next_token_chooser,
            stopping_criteria,
            all_input_ids,
        ) in enumerate(iterator):
            # Select next token
            next_token_id, logprobs = next_token_chooser(
                all_input_ids.view(1, -1), logits[-1:, :]
            )

            # Append next token to all tokens
            all_input_ids = torch.cat([all_input_ids, next_token_id])
            new_input_length = input_length + 1

            # Generated token
            next_token_logprob = logprobs[-1, next_token_id]
            next_token_id_squeezed = next_token_id.squeeze()
            next_token_text, prefix_offset, read_offset = self.decode_token(
                all_input_ids[:, 0], prefix_offset, read_offset
            )

            # Evaluate stopping criteria
            stop, reason = stopping_criteria(
                next_token_id_squeezed,
                next_token_text,
            )

            if not stop:
                stopped = False

            # Shard generations
            # All generations will be appended in the rust sharded client
            if i % self.world_size == self.rank:
                if stop:
                    # Decode generated tokens
                    output_text = self.decode(
                        all_input_ids[-stopping_criteria.current_tokens :, 0]
                    )
                    # Get seed
                    if isinstance(next_token_chooser.choice, Sampling):
                        seed = next_token_chooser.choice.seed
                    else:
                        seed = None

                    generated_text = GeneratedText(
                        output_text, stopping_criteria.current_tokens, reason, seed
                    )
                else:
                    generated_text = None

                # Prefill
                if stopping_criteria.current_tokens == 1 and request.prefill_logprobs:
                    # Remove generated token to only have prefill and add nan for first prompt token

                    prefill_logprobs = [float("nan")] + torch.log_softmax(
                        logits, -1
                    ).gather(1, all_input_ids[1:]).squeeze(1)[
                        -new_input_length:-1
                    ].tolist()
                    prefill_token_ids = all_input_ids[-new_input_length:-1]
                    prefill_texts = self.tokenizer.batch_decode(
                        prefill_token_ids,
                        clean_up_tokenization_spaces=False,
                        skip_special_tokens=False,
                    )
                    prefill_tokens = PrefillTokens(
                        prefill_token_ids, prefill_logprobs, prefill_texts
                    )
                else:
                    prefill_tokens = None

                generation = Generation(
                    request.id,
                    prefill_tokens,
                    next_token_id_squeezed,
                    next_token_logprob,
                    next_token_text,
                    next_token_id_squeezed.item() in self.all_special_ids,
                    generated_text,
                )

                generations.append(generation)

            # Update values
            batch.input_ids[i, 0] = next_token_id
            batch.all_input_ids[i] = all_input_ids
            batch.input_lengths[i] = new_input_length
            batch.prefix_offsets[i] = prefix_offset
            batch.read_offsets[i] = read_offset
            batch.max_input_length = max(batch.max_input_length, new_input_length)

        # We finished all generations in the batch; there is no next batch
        if stopped:
            return generations, None

        # Slice unused values from prefill
        batch.input_ids = batch.input_ids[:, :1]

        # Update attention_mask as we added a new token to input_ids
        batch.attention_mask[:, -batch.padding_right_offset] = 1
        # Decrease right offset
        batch.padding_right_offset -= 1

        # Update position_ids
        batch.position_ids = batch.position_ids[:, -1:] + 1

        # Update past key values
        batch.past_key_values = past

        return generations, batch
