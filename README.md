# Text Generation Inference

This is Preemo's fork of `text-generation-inference`, originally developed by Hugging Face. The original README is at [README-HuggingFace.md](README-HuggingFace.md). Since Hugging Face's `text-generation-inference` is no longer open-source, we have forked it and will continue to develop it here.

Our goal is to create an open-source text generation inference server that is modularized to allow for easy add state-of-the-art models, functionalities and optimizations. Functionalities and optimizations should be composable, so that users can easily combine them to create a custom inference server that fits their needs.

## our plan

We at Preemo are currently busy working on our first release of our other product, so we expect to be able to start open-source development on this repository in September 2023. We will be working on the following, to ease the external contributions:

- [ ] Adding a public visible CI/CD pipeline that runs tests and builds docker images
- [ ] Unifying the build tools
- [ ] Modularizing the codebase, introducing a plugin system

Our long-term goal is to grow the community around this repository, as a playground for trying out new ideas and optimizations in LLM inference. We at Preemo will implement features that interest us, but we also welcome contributions from the community, as long as they are modularized and composable.
