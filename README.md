# uwu mew mew v6
a cute uwu bot

---

## Installation:
1. clone the repository
   
    `git clone https://github.com/YuraSuper2048/uwu-mew-mew-6.git`
2. set the environment variables

    ```shell
    export DISCORD_TOKEN= # discord bot token
    export GPT_OPENAI_API_BASE= # api base ending with /v1
    export GPT_OPENAI_API_KEY= # api key
    export CLAUDE_OPENAI_API_BASE= # api base, in openai spec, ending with /v1
    export CLAUDE_OPENAI_API_KEY= # api key
    export RUST_LOG=error,uwu_mew_mew_6=info # replace info with trace if you want to get all logging
    ```
3. run
    
    `cargo run`
    
    or

    `cargo run --release`

    for an optimized build

## Notice
if your going to host this, please understand the [license conditions](LICENSE.txt). This work is licensed under AGPLv3.