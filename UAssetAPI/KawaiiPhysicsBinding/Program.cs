using UAssetAPI;
using UAssetAPI.UnrealTypes;
using UAssetAPI.Unversioned;

internal static class Program
{
    private static int Main(string[] args)
    {
        if (args.Length < 2)
        {
            return Usage();
        }

        string command = args[0];

        if (!string.Equals(command, "port", StringComparison.OrdinalIgnoreCase))
        {
            return Usage();
        }

        return Port(args);
    }

    private static int Usage()
    {
        Console.Error.WriteLine("Usage:");
        Console.Error.WriteLine("  KawaiiPhysicsBinding port [usmap_path] <uasset_path> [--force-rebuild]");
        Console.Error.WriteLine("  KawaiiPhysicsBinding port <uasset_path> [--force-rebuild]");
        return 2;
    }

    private static int Port(string[] args)
    {
        string? usmapPath = null;
        string? uassetPath = null;

        var options = new KawaiiPhysicsPortOptions();

        for (int i = 1; i < args.Length; i++)
        {
            string arg = args[i];

            if (arg.StartsWith("--", StringComparison.Ordinal))
            {
                switch (arg)
                {
                    case "--force-rebuild":
                    case "--force-rebuild-chain0":
                        options.ForceRebuildChain0 = true;
                        break;

                    default:
                        Console.Error.WriteLine($"[KawaiiPhysicsBinding] Unknown option: {arg}");
                        return 2;
                }

                continue;
            }

            if (usmapPath == null)
            {
                usmapPath = arg;
            }
            else if (uassetPath == null)
            {
                uassetPath = arg;
            }
            else
            {
                Console.Error.WriteLine($"[KawaiiPhysicsBinding] Unexpected positional arg: {arg}");
                return 2;
            }
        }

        if (uassetPath == null)
        {
            uassetPath = usmapPath;
            usmapPath = null;
        }

        if (LooksLikeUsmapPath(uassetPath) && IsUassetPath(usmapPath))
        {
            (usmapPath, uassetPath) = (uassetPath, usmapPath);
        }

        if (string.IsNullOrWhiteSpace(uassetPath))
        {
            Console.Error.WriteLine("[KawaiiPhysicsBinding] UAsset path was null or empty");
            return 2;
        }

        if (!File.Exists(uassetPath))
        {
            Console.Error.WriteLine($"[KawaiiPhysicsBinding] UAsset not found: {uassetPath}");
            return 2;
        }

        if (!string.IsNullOrWhiteSpace(usmapPath) && !File.Exists(usmapPath))
        {
            Console.Error.WriteLine($"[KawaiiPhysicsBinding] USMAP not found, continuing without mappings: {usmapPath}");
            usmapPath = null;
        }

        try
        {
            Console.Error.WriteLine($"[KawaiiPhysicsBinding] Loading asset: {uassetPath}");
            Console.Error.WriteLine($"[KawaiiPhysicsBinding] USMAP path: {(string.IsNullOrWhiteSpace(usmapPath) ? "null" : usmapPath)}");

            var asset = LoadAssetLikeCli(uassetPath, usmapPath);

            var result = KawaiiPhysicsLegacyPorter.PortLegacyAnimNodes(asset, options);

            if (result.PortedAnimNodes > 0)
            {
                asset.Write(uassetPath);
                Console.Error.WriteLine($"[KawaiiPhysicsBinding] Patched: {uassetPath}");
            }

            Console.Error.WriteLine(
                $"[KawaiiPhysicsBinding] visited={result.VisitedAnimNodes} ported={result.PortedAnimNodes} skipped_existing={result.SkippedExistingChains}"
            );

            Console.Error.WriteLine($"Visited AnimNodes: {result.VisitedAnimNodes}");
            Console.Error.WriteLine($"Ported AnimNodes: {result.PortedAnimNodes}");
            Console.Error.WriteLine($"Skipped Existing Chains: {result.SkippedExistingChains}");

            return 0;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine("[KawaiiPhysicsBinding] failed:");
            Console.Error.WriteLine(ex.ToString());
            return 1;
        }
    }

    private static UAsset LoadAssetLikeCli(string uassetPath, string? usmapPath)
    {
        try
        {
            var asset = new UAsset(uassetPath, EngineVersion.VER_UE5_3)
            {
                UseSeparateBulkDataFiles = true
            };

            Console.Error.WriteLine(
                $"[KawaiiPhysicsBinding] Loaded without mappings: HasUnversionedProperties={asset.HasUnversionedProperties}, Exports={asset.Exports.Count}"
            );

            return asset;
        }
        catch (Exception noMappingsEx)
        {
            Console.Error.WriteLine("[KawaiiPhysicsBinding] No-mapping load failed:");
            Console.Error.WriteLine(noMappingsEx.ToString());

            if (string.IsNullOrWhiteSpace(usmapPath))
            {
                throw;
            }
        }

        try
        {
            Console.Error.WriteLine($"[KawaiiPhysicsBinding] Retrying with USMAP: {usmapPath}");

            var mappings = new Usmap(usmapPath);

            var asset = new UAsset(uassetPath, EngineVersion.VER_UE5_3, mappings)
            {
                UseSeparateBulkDataFiles = true
            };

            Console.Error.WriteLine(
                $"[KawaiiPhysicsBinding] Loaded with mappings: HasUnversionedProperties={asset.HasUnversionedProperties}, Exports={asset.Exports.Count}"
            );

            return asset;
        }
        catch (Exception mappedEx)
        {
            throw new InvalidOperationException(
                $"Failed to load asset with or without mappings: {uassetPath}",
                mappedEx
            );
        }
    }

    private static bool LooksLikeUsmapPath(string? path)
    {
        return !string.IsNullOrWhiteSpace(path)
            && path.EndsWith(".usmap", StringComparison.OrdinalIgnoreCase);
    }

    private static bool IsUassetPath(string? path)
    {
        return !string.IsNullOrWhiteSpace(path)
            && path.EndsWith(".uasset", StringComparison.OrdinalIgnoreCase);
    }
}