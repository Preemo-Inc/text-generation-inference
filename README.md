# Text Generation Inference

This is Preemo's fork of `text-generation-inference`, originally developed by Hugging Face. The original README is at [README-HuggingFace.md](README-HuggingFace.md). Since Hugging Face's `text-generation-inference` is no longer open-source, we have forked it and will continue to develop it here.


Our goal is to create an open-source text generation inference server that is modularized to allow for easy add state-of-the-art models, functionalities and optimizations. Functionalities and optimizations should be composable, so that users can easily combine them to create a custom inference server that fits their needs.

## our plan

We at Preemo are currently busy working on our first release of our other product, so we expect to be able to start open-source development on this repository in September 2023. We will be working on the following, to ease the external contributions:

- [ ] Adding a public visible CI/CD pipeline that runs tests and builds docker images
- [ ] Unifying the build tools
- [ ] Modularizing the codebase, introducing a plugin system

Our long-term goal is to grow the community around this repository, as a playground for trying out new ideas and optimizations in LLM inference. We at Preemo will implement features that interest us, but we also welcome contributions from the community, as long as they are modularized and composable.

## Extra features in comparison to Hugging Face `text-generation-inference` v0.9.4

### 4bit quantization

4bit quantization is available using the [NF4 and FP4 data types from bitsandbytes](https://arxiv.org/pdf/2305.14314.pdf). It can be enabled by providing `--quantize bitsandbytes-nf4` or `--quantize bitsandbytes-fp4` as a command line argument to `text-generation-launcher`.

### CTranslate2

Int8 Ctranslate2 quantization is available using the `--quantize ct2` as a command line argument to `text-generation-launcher`. It will convert the PyTorch Model provided in `--model-id` on the fly, and save the quantized model for the next start-up for up to 10x faster loading times. If CUDA is not available, Ctranslate2 will default to run on CPU.

### Chat Completions in OpenAI Format

`/chat/completions` and `/completions` endpoints are available, using the API schema commonly known from OpenAI.
You may set the `TGICHAT_(USER|ASS|SYS)_(PRE|POST)` environment variables, to wrap the chat messages.

<details>
  <summary>Optimal Llama-2-Chat config</summary>
  For Llama-2, you should wrap each chat message with a different strings, depending on the role.
  Supported roles are `assistant`, `user`, `system`.
  
  ```bash
  TGICHAT_USER_PRE=" [INST] "
  TGICHAT_USER_POST=" [\\INST] "
  TGICHAT_ASS_PRE=""
  TGICHAT_ASS_POST=""
  TGICHAT_SYS_PRE=" [INST] <<SYS>> "
  TGICHAT_SYS_POST=" <</SYS>> [\\INST] "
  ```

  Note: To access a gated model, you may need to set: `HUGGING_FACE_HUB_TOKEN` for your access token.
  
</details>

## Get started with Docker

```bash
model=TheBloke/Llama-2-13B-Chat-fp16 # around 14GB Vram.
volume=$PWD/data # share a volume with the Docker container to avoid downloading weights every run
image=docker.io/michaelf34/tgi:03-10-2023 # docker image by @michaelfeil

docker run --gpus all --shm-size 1g -p 8080:80 -v $volume:/data $image --model-id $model --quantize ct2
```

To see all options of `text-generation-launcher` you may use the `--help` command: 
```bash
docker run $image --help
```
