<h1 align="center">fishy</h1>

<div align="center">
  <img src="https://raw.githubusercontent.com/p2panda/.github/main/assets/fish-left.gif" width="auto" height="35px">
  <strong>Create, manage and deploy p2panda schemas</strong>
  <img src="https://raw.githubusercontent.com/p2panda/.github/main/assets/fish-right.gif" width="auto" height="35px">
</div>

<br />

<div align="center">
  <a href="https://www.youtube.com/watch?v=buJZeQAmrq8" target="_blank">
    <img src="https://raw.githubusercontent.com/p2panda/fishy/main/example.gif" width="600" />
  </a>
</div>

<div align="center">
  <h3>
    <a href="https://github.com/p2panda/fishy/releases">
      Releases
    </a>
    <span> | </span>
    <a href="https://p2panda.org/about/contribute">
      Contribute
    </a>
    <span> | </span>
    <a href="https://p2panda.org">
      Website
    </a>
  </h3>
</div>

<br/>

Command-line-tool to easily create update and share your [`p2panda`] schemas.

`fishy` parses your current version of your schemas and matches it with
previous ones to calculate the difference and apply changes automatically.

Your schema changes are committed to a `schema.lock` file which you can share
with other developers. With `fishy` they will be able to deploy the same schema
on their nodes.

## Usage

```
Create, manage and deploy p2panda schemas

Usage: fishy <COMMAND>

Commands:
  init    Initialises all files for a new fishy project in a given folder
  build   Automatically creates and signs p2panda data from a key pair and the defined schemas
  deploy  Deploy created schemas on a node
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Examples

```bash
# Initialise a new schema, this creates a `schema.toml` file you can edit
fishy init

# Same as above, but in a different folder and with the name already defined
fishy init -n icecream ~/dev/schemas

# Commit any changes to the schema, this updates your `schema.lock` file
fishy build

# Only inspect the current status of your schemas, do not commit anything
fishy build --inspect

# Deploy commits to external node
fishy deploy --endpoint http://localhost:2020/graphql
```

## Install

### Pre-compiled binaries

Check out our [Releases](https://github.com/p2panda/fishy/releases) section.

### Compile it yourself

For the following steps you need a
[Rust](https://www.rust-lang.org/learn/get-started) development environment on
your machine.


```bash
# Download source code
git clone https://github.com/p2panda/fishy.git
cd fishy

# Compile binary
cargo build --release

# Copy binary into your path (for example)
cp ./target/release/fishy ~/.local/bin
```

## Tutorial

1. Initialise a new schema by running `fishy init`. A dialogue will ask you for
   the name of your first schema. Enter a name, for example `cafe` and press
   enter. You will now find a `schema.toml` and `secret.txt` file in
   your folder.
2. Edit `schema.toml` with any text editor. Follow the format to specify
   multiple schemas, their fields, types and relations to each other. For
   example:
   ```toml
   [cafe]
   description = "A list of cafes"

   [cafe.fields]
   name = { type = "str" }
   address = { type = "str" }
   opening_year = { type = "int" }

   [icecream]
   description = "Icecream sorts you can get in cafes"

   [icecream.fields]
   name = { type = "str" }
   sweetness = { type = "str" }
   cafes = { type = "relation_list", schema = { name = "cafe" } }
   ```
3. You can commit these changes now to `schema.lock` by running `fishy build`.
   The tool will automatically show you the changes which will be committed and
   ask for your confirmation. Hit `y` to confirm. This step will generate,
   encode and sign the commits with your private key stored in `secret.txt`.
4. This version of the schema lives now in `schema.lock`. You can go back to
   the `schema.toml` file and do any changes to the schema, run `fishy build`
   again to apply them. The tool will again only show you exactly what you've
   changed and generate the commits for only exactly these changes. Try it out!
5. Finally deploy the schema on one or many nodes by running `fishy deploy`.
   Make sure you have a [node](https://github.com/p2panda/aquadoggo) running
   somewhere.
6. Share the `schema.lock` file with others, with it they will be able to
   deploy the schemas on their nodes!

## License

GNU Affero General Public License v3.0 [`AGPL-3.0-or-later`](LICENSE)

## Supported by

<img src="https://raw.githubusercontent.com/p2panda/.github/main/assets/ngi-logo.png" width="auto" height="80px"><br />
<img src="https://raw.githubusercontent.com/p2panda/.github/main/assets/nlnet-logo.svg" width="auto" height="80px"><br />
<img src="https://raw.githubusercontent.com/p2panda/.github/main/assets/eu-flag-logo.png" width="auto" height="80px">

*This project has received funding from the European Union’s Horizon 2020
research and innovation programme within the framework of the NGI-POINTER
Project funded under grant agreement No 871528*

[`p2panda`]: https://p2panda.org
