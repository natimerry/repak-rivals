## Which Download Should I Use?

Each release has two builds for each platform:

- Normal: smaller download, faster KawaiiPhysics porting, requires a local .NET runtime.
- Self-contained: larger download, named with `self-contained`, bundles the KawaiiPhysics .NET helper so users do not need to install .NET separately.

Use the normal build if you already have .NET installed. Use the self-contained build if KawaiiPhysics fails with a .NET, hostfxr, or runtime dependency error.

Latest .NET Runtime download:
https://dotnet.microsoft.com/download/dotnet/latest/runtime

Linux package names:

- Arch: `dotnet-runtime`, `dotnet-hostfxr`
- Ubuntu: `dotnet-runtime-8.0`
- Fedora: `dotnet-runtime-8.0`
