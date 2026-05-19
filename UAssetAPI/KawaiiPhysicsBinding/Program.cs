using UAssetAPI;
using UAssetAPI.UnrealTypes;
using UAssetAPI.Unversioned;

static int Usage()
{
    Console.Error.WriteLine("Usage: KawaiiPhysicsBinding port <usmap_path> <uasset_path> [--force-rebuild]");
    return 2;
}

if (args.Length < 3 || !string.Equals(args[0], "port", StringComparison.OrdinalIgnoreCase))
{
    return Usage();
}

string usmapPath = args[1];
string uassetPath = args[2];
bool forceRebuild = args.Skip(3).Any(x => x == "--force-rebuild" || x == "--force-rebuild-chain0");

if (!File.Exists(usmapPath))
{
    Console.Error.WriteLine($"USMAP not found: {usmapPath}");
    return 2;
}

if (!File.Exists(uassetPath))
{
    Console.Error.WriteLine($"UAsset not found: {uassetPath}");
    return 2;
}

try
{
    var mappings = new Usmap(usmapPath);
    var asset = new UAsset(uassetPath, EngineVersion.VER_UE5_3, mappings)
    {
        UseSeparateBulkDataFiles = true
    };

    var result = KawaiiPhysicsLegacyPorter.PortLegacyAnimNodes(asset, new KawaiiPhysicsPortOptions
    {
        ForceRebuildChain0 = forceRebuild
    });

    if (result.PortedAnimNodes > 0)
    {
        asset.Write(uassetPath);
    }

    Console.Error.WriteLine(
        $"[KawaiiPhysicsBinding] visited={result.VisitedAnimNodes} ported={result.PortedAnimNodes} skipped_existing={result.SkippedExistingChains}");
    return 0;
}
catch (Exception ex)
{
    Console.Error.WriteLine($"[KawaiiPhysicsBinding] failed: {ex.Message}");
    return 1;
}
