# Project, Trademark, and Privacy Notes

This page describes the current UMC project's intended ownership boundaries and software behavior. It is not a legal opinion. These notes should be updated when UMC adds account authentication, telemetry, hosted services, mod distribution, or other material data flows.

## Project relationship to Minecraft

UMC (Universal Minecraft Core) is an independent community software project. It is not made, approved, endorsed, or supported by Mojang Studios or Microsoft, and it is not an official Minecraft product. The name “Minecraft” identifies the game UMC is designed to work with; it does not indicate ownership, partnership, or endorsement.

Use a clear notice in project listings, release descriptions, and application About/Legal pages:

> UMC is an independent project. It is not an official Minecraft product and is not approved by or associated with Mojang or Microsoft.

Mojang and Microsoft retain their rights in the Minecraft name, marks, logos, game, launcher, and related materials. UMC integrations should not use their logos or presentation in a way that suggests official status. Review the current [Minecraft Usage Guidelines](https://www.minecraft.net/en-us/usage-guidelines) and [Minecraft EULA](https://www.minecraft.net/en-us/eula) before distributing or promoting a Minecraft-related product.

## What belongs to whom

| Material | Project boundary |
|---|---|
| UMC-authored source code and documentation | Copyright is held by the listed UMC authors/contributors and licensed under the MIT License in this repository. |
| UMC name and project artwork | Used to identify this independent project. Permission to use UMC branding should not be presented as permission to use Minecraft or Mojang/Microsoft branding. |
| Minecraft game files, Minecraft/Mojang/Microsoft marks, and their online services | Not owned or licensed by UMC. UMC does not bundle the game in its releases; the current engine obtains game files from metadata and download sources as part of user-requested installation. |
| Microsoft, Xbox, and Minecraft account credentials and profile information | Not UMC property. Microsoft authentication is not implemented in the current version. Future support must explain what is exchanged and stored and must not collect the user's Microsoft password. |
| Fabric, Forge, NeoForge, mod platforms, mods, and modpacks | Independently maintained or created. Their names, code, content, trademarks, terms, and licenses remain with their respective owners. UMC's adapters do not grant rights to those projects or content. |
| Rust dependencies | Third-party software governed by the licenses declared by their authors. Release packaging should include applicable notices; the UMC MIT license does not replace dependency licenses. |

MIT applies to UMC's own covered software. It does not grant rights to Minecraft files, third-party services, mods, or trademarks. “Powered by UMC Core” is an optional integration credit, not an endorsement and not an MIT requirement. See the [developer guide](developer-guide.md) for redistribution notes.

## Privacy summary for the current version

- UMC does not operate a user account service, cloud sync service, telemetry endpoint, or remote management service in the current version.
- UMC stores managed game files, instance settings, task/process state, and logs locally beneath `UMC_DATA_DIR` (or `./umc-data` in development). Logs may contain player names, local paths, game output, or mod output.
- Version lookup, game installation, Java runtime installation, and loader metadata lookup make direct HTTPS requests to the relevant upstream services. Those services may receive ordinary connection metadata, including the client's IP address.
- The daemon listens only on local IPC endpoints. Access to the data directory and its socket/pipe grants local control; users should keep that directory private.
- Microsoft account authentication is not implemented. This version does not store Microsoft passwords, OAuth tokens, or refresh tokens.

Users should inspect logs before sharing them and should not publish account credentials or private server details printed by games or mods. If an integrating GUI adds telemetry, account storage, crash reporting, or cloud features, its publisher must separately explain those features.

## Future account support

Before Microsoft authentication is released, UMC should update this notice and the user guide with the actual registered client identity, sign-in flow, service endpoints, token storage mechanism, refresh/logout behavior, account deletion steps, and any data retained by UMC. Tokens must not be written to ordinary instance configuration or logs. Account sign-in will not make UMC a Microsoft or Mojang service, and users remain subject to the terms of the relevant services.

## Warranty and support boundary

UMC is provided under the MIT License, including its “AS IS” warranty and liability disclaimer. Mojang/Microsoft and third-party mod or loader maintainers do not provide support for UMC solely because it interacts with their software or services. Report UMC-specific issues through the project's GitHub issue tracker; game, account, loader, and mod issues may need to be directed to their respective maintainers.
