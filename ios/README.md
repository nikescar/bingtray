
## Local development

1. Build locally via `just build`
2. Change the `Package.swift` to use the locally built `xcframework`
3. Switch XCode package dependency to use local repo via path
4. Patch `Cargo.toml` to use local repository

## Bevy version support

|bevy|crate|
|---|---|
|0.16|0.6,main|
|0.15|0.5|
|0.14|0.3,0.4|
|0.13|0.2|
