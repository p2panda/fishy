<h1 align="center">fishy</h1>

<div align="center">
  <strong>Create, manage and deploy p2panda schemas</strong>
</div>

<br />

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

Command-line-tool to easily create and update your [`p2panda`] schemas and
deploy them on a node. 

`fishy` calculates the current version of your schemas and matches it with the
previous versions, it shows the difference and applies the changes
automatically.

Your schema changes are committed to a `schema.lock` file which you can now
give to others. With `fishy` they will be able to deploy the same schema on
their nodes.

## Usage

```
Create, manage and deploy p2panda schemas

Usage: fishy <COMMAND>

Commands:
  init    Initialises all files for a new fishy project in a given folder
  update  Automatically creates and signs p2panda data from a key pair and the defined schemas
  deploy  Deploy created schemas on a node
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Examples

```bash
# Initialise a new schema, this will create a `schema.toml` file you can edit
fishy init

# Same as above, but in a different folder and with the name already defined
fishy init -n icecream ~/dev/schemas

# Commit any changes to the schema, this updates your `schema.lock` file
fishy update

# Only inspect the current status of your schemas, do not commit anything
fishy update --inspect

# Deploy commits to external node
fishy deploy --endpoint http://localhost:2020/graphql
```

## Tutorial

1. Initialise a new schema by running `fishy init`. A dialogue will ask you for
   the name of your first schema. Enter a name, for example `cafe` and press
   enter. You will now find a `schema.toml` file and a `secret.txt` file in
   your folder.
2. Edit the `schema.toml` file with any text editor. Follow the format to
   specify multiple schemas, their fields, types and relations to each other.
   For example:
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
3. You can commit these changes now to a `schema.lock` file by running `fishy
   update`. The tool will automatically show you the changes which will be
   committed and ask for your confirmation. Hit `y` to confirm. This step will
   generate, encode and sign the commits with your private key stored in
   `secret.txt`.
4. This version of the schema sits now in the `schema.lock` file. You can go
   back to the `schema.toml` file and do any changes to the schema, run `fishy
   update` again to apply them. The tool will again only show you exactly what
   you've changed and generate the commits for only exactly these changes. Try
   it out!
5. You can also run `fishy update --inspect` if you don't want to committ
   anything but you're curious about the current state of your schemas. Also it
   is a good way to read the schema id's which are required for development of
   your p2panda clients.
6. Finally deploy the schema on one or many nodes by running `fishy deploy`.
   Make sure you have a [node](https://github.com/p2panda/aquadoggo) running
   somewhere.
7. You can share the `schema.lock` file with others, they will be able now to
   deploy the schema's on their machines with this file.

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
