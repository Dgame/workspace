Create a `workspace.toml` wherever you want and add the git-projects you want to have as shown in the basic example below:

```toml
[[workspace]]
provider = "github"
name = "<user>/<php-git-project>"

[[workspace]]
provider = "github"
name = "<user>/<rust-git-project>"
```

Then you can do either
 - `pull`: Pull all cloned repositories
 - `clone`: Clone all not cloned repositories
 - `fetch`: Fetch all cloned repositories
 - `sync`: Pull all cloned repositories, clone all not cloned repositories
