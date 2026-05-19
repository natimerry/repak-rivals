using System;
using System.Collections.Generic;
using System.IO;
using UAssetAPI.UnrealTypes;

namespace UAssetAPI.ExportTypes
{
    /// <summary>
    /// Export for StaticMesh assets with proper FStaticMaterial parsing.
    /// </summary>
    public class StaticMeshExport : NormalExport
    {
        /// <summary>
        /// Parsed materials from the mesh. If null, materials weren't found/parsed.
        /// </summary>
        public List<FStaticMaterial> Materials;
        
        /// <summary>
        /// Offset in Extras where materials array starts (for reconstruction).
        /// </summary>
        private int _materialsOffset = -1;
        
        /// <summary>
        /// Original materials byte length before parsing.
        /// </summary>
        private int _originalMaterialsByteLength = 0;

        public StaticMeshExport(Export super) : base(super)
        {
        }

        public StaticMeshExport(UAsset asset, byte[] extras) : base(asset, extras)
        {
        }

        public StaticMeshExport()
        {
        }

        public override void Read(AssetBinaryReader reader, int nextStarting)
        {
            base.Read(reader, nextStarting);
            
            // After base.Read(), Extras contains the remaining binary data
            // Try to parse materials from Extras
            if (Extras != null && Extras.Length > 0)
            {
                TryParseMaterials();
            }
        }

        /// <summary>
        /// Try to find and parse the FStaticMaterial array from Extras.
        /// StaticMaterials are typically near the end of the file after render data.
        /// </summary>
        private void TryParseMaterials()
        {
            const int MAX_MATERIAL_COUNT = 50;
            const int MATERIAL_STRUCT_SIZE = 36; // FStaticMaterial: FPackageIndex(4) + FName(8) + FPackageIndex(4) + FMeshUVChannelInfo(20)
            
            // Search from near the end of the file
            int searchStart = Math.Max(4, Extras.Length - 2000);
            
            for (int i = searchStart; i < Extras.Length - 4; i++)
            {
                int potentialCount = BitConverter.ToInt32(Extras, i);
                if (potentialCount < 1 || potentialCount > MAX_MATERIAL_COUNT)
                    continue;
                
                // Check if next bytes look like an FPackageIndex (negative value for import)
                if (i + 4 >= Extras.Length)
                    continue;
                    
                int firstPkgIdx = BitConverter.ToInt32(Extras, i + 4);
                if (firstPkgIdx >= 0 || firstPkgIdx < -1000)
                    continue;
                
                // Check if the expected end of materials array is near the end of file
                int expectedEnd = i + 4 + (potentialCount * MATERIAL_STRUCT_SIZE);
                if (expectedEnd < Extras.Length - 100 || expectedEnd > Extras.Length + 100)
                    continue;
                
                // Verify by checking if it looks like FStaticMaterial
                bool validPattern = true;
                for (int m = 0; m < Math.Min(potentialCount, 3); m++)
                {
                    int matOffset = i + 4 + (m * MATERIAL_STRUCT_SIZE);
                    if (matOffset + 4 > Extras.Length)
                    {
                        validPattern = false;
                        break;
                    }
                    
                    int pkgIdx = BitConverter.ToInt32(Extras, matOffset);
                    if (pkgIdx >= 0 || pkgIdx < -1000)
                    {
                        validPattern = false;
                        break;
                    }
                }
                
                if (!validPattern)
                    continue;
                
                // Found materials - parse them
                _materialsOffset = i;
                int materialCount = potentialCount;
                _originalMaterialsByteLength = 4 + (materialCount * MATERIAL_STRUCT_SIZE);
                
                // Only parse if we have enough data
                if (_materialsOffset + _originalMaterialsByteLength > Extras.Length)
                    continue;
                
                Materials = new List<FStaticMaterial>();
                using var ms = new MemoryStream(Extras, i + 4, Math.Min(Extras.Length - i - 4, materialCount * MATERIAL_STRUCT_SIZE));
                using var matReader = new AssetBinaryReader(ms, Asset);
                
                try
                {
                    for (int m = 0; m < materialCount; m++)
                    {
                        var mat = new FStaticMaterial();
                        mat.Read(matReader);
                        Materials.Add(mat);
                    }
                }
                catch
                {
                    // Failed to parse, reset
                    Materials = null;
                    _materialsOffset = -1;
                }
                
                break;
            }
        }

        public override void Write(AssetBinaryWriter writer)
        {
            base.Write(writer);
            
            // StaticMesh materials don't need FGameplayTagContainer padding for Marvel Rivals
            // But we keep the structured parsing for future flexibility
        }
    }
}
