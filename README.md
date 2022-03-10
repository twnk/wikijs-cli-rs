# Wikcli 

A very simple utility for bulk operations on Wiki pages

USAGE:
    wiki [OPTIONS] <SUBCOMMAND>

OPTIONS:
        --api-key <API_KEY>
            GraphQL API Key

        --color <COLOR>
            Color [default: auto] [possible values: always, auto, never]

        --config <CONFIG>
            Config File

        --endpoint <ENDPOINT>
            GraphQL Endpoint

    -h, --help
            Print help information

        --no-force-https
            HTTPS (Default On)

        --no-http2-prior-knowledge
            HTTP2 (Default On)

    -v, --verbose
            Verbosity level (can be specified multiple times)

    -V, --version
            Print version information

SUBCOMMANDS:
    config    Generate config file
    help      Print this message or the help of the given subcommand(s)
    list      List wiki pages by path prefix
    move      Move wiki pages to a new path

## Config

Use `wiki config --interactive`, and wikcli will prompt for the API key (input masked), GraphQL endpoint, and whether to default to https & http2. 
When using `wiki list` or `wiki move`, you can use `-t tag -t tag2` to restrict the pages listed/moved to pages which have specific tags. 
Finally, use `wiki move [prefix] -d destination`, e.g. `wiki move helpdesk/2021 -d archive/helpdesk/2021` to move all pages beginning with `helpdesk/2021` to the new path. 

Partial paths are acceptable, e.g. If you had a number of similarly named directories you wanted to turn into subfolders, such as `tools-deploy/`, `tools-monitoring` and you wanted them to be `tools/deploy` etc, then `wiki move tools- -d tools/` would rewrite the paths correctly. 
