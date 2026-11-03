# Mod Center integration

Status: implementation prepared on `feature/mod-center-foundation`.

## Android mod source

- The Android **Mods** tab reads only the `Geko7490/MinecraftMod` catalogue.
- The catalogue JSON is converted into the existing Horizon index model.
- The existing .hmod verifier checks the downloaded file's exact size, SHA-256, package manifest, API and capabilities before installation.
- The Horizon legacy registry remains available for native shader packs.
- Modrinth and configured CurseForge remain separate sources for resource packs and supported browsing; they are no longer mod sources on Android.
- Arbitrary HTTPS .hmod URL import has been removed from the Mods search field. Local file import remains available.
- Java Fabric/Forge/NeoForge .jar code is not executed by this Android client.

## Mod developers

The companion repository contains the Ruby content-mod example, catalog schema, Python .hmod builder, release automation and Turkish modding guide:

https://github.com/Geko7490/MinecraftMod

Merging the example manifest to `main` builds a .hmod release and updates the catalogue automatically. The Android app reads the catalogue from the repository's raw JSON URL.

## Validation

- Android source-check workflow.
- Android arm64 APK workflow.
- MinecraftMod CI builds and validates the Ruby example package.
