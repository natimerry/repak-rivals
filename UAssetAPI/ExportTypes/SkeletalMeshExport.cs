using System;
using System.Collections.Generic;
using System.IO;
using UAssetAPI.UnrealTypes;

namespace UAssetAPI.ExportTypes
{
    /// <summary>
    /// Export for SkeletalMesh assets with comprehensive extra data parsing.
    /// Parses FStripDataFlags, FBoxSphereBounds, FSkeletalMaterial[], FReferenceSkeleton, and LOD data.
    /// Handles FGameplayTagContainer padding for Marvel Rivals compatibility.
    /// </summary>
    public class SkeletalMeshExport : NormalExport
    {
        #region Parsed Extra Data Fields

        /// <summary>
        /// Strip data flags indicating what data was stripped during cooking.
        /// </summary>
        public FStripDataFlags StripFlags;

        /// <summary>
        /// Imported bounding box and sphere for the mesh.
        /// </summary>
        public FBoxSphereBounds ImportedBounds;

        /// <summary>
        /// Parsed materials from the mesh. If null, materials weren't found/parsed.
        /// </summary>
        public List<FSkeletalMaterial> Materials;

        /// <summary>
        /// Reference skeleton containing bone hierarchy and reference pose.
        /// </summary>
        public FReferenceSkeleton ReferenceSkeleton;

        /// <summary>
        /// Whether the mesh is cooked (has render data).
        /// </summary>
        public bool bCooked;

        /// <summary>
        /// Number of LOD models in the mesh.
        /// </summary>
        public int LODCount;

        /// <summary>
        /// Remaining unparsed data after the known structures.
        /// This contains LOD render data which is complex and version-dependent.
        /// </summary>
        public byte[] RemainingExtraData;

        #endregion

        #region Configuration

        /// <summary>
        /// Whether to include FGameplayTagContainer when writing materials.
        /// Set to true for Marvel Rivals compatibility.
        /// </summary>
        public bool IncludeGameplayTags = true;

        /// <summary>
        /// Whether extra data was successfully parsed.
        /// </summary>
        public bool ExtraDataParsed { get; private set; } = false;

        #endregion

        #region Internal State

        private int _materialsOffset = -1;
        private int _originalMaterialsByteLength = 0;
        private int _parsedDataEndOffset = 0;
        private bool _sourceHadGameplayTags = false;

        /// <summary>
        /// Whether the source file already had FGameplayTagContainer in materials.
        /// If true, the file was extracted from game and already has correct format - no re-serialization needed.
        /// </summary>
        public bool SourceHadGameplayTags => _sourceHadGameplayTags;

        #endregion

        #region Constructors

        public SkeletalMeshExport(Export super) : base(super)
        {
        }

        public SkeletalMeshExport(UAsset asset, byte[] extras) : base(asset, extras)
        {
        }

        public SkeletalMeshExport()
        {
        }

        #endregion

        #region Read/Write

        public override void Read(AssetBinaryReader reader, int nextStarting)
        {
            base.Read(reader, nextStarting);
            // Note: Extras is populated AFTER Read() returns, in UAsset.ConvertExportToChildExportAndRead
            // So we can't parse here - we use lazy parsing in EnsureExtraDataParsed()
        }
        
        /// <summary>
        /// Ensures extra data has been parsed. Call this before accessing Materials or other parsed data.
        /// </summary>
        public void EnsureExtraDataParsed()
        {
            if (_extraDataParseAttempted) return;
            _extraDataParseAttempted = true;
            
            if (Extras != null && Extras.Length > 0)
            {
                TryParseExtraData();
            }
        }
        
        private bool _extraDataParseAttempted = false;

        /// <summary>
        /// Parse the extra data section which contains mesh-specific binary data.
        /// Structure (per CUE4Parse USkeletalMesh.cs):
        /// 1. FStripDataFlags (2 bytes)
        /// 2. FBoxSphereBounds (28 or 56 bytes depending on LWC)
        /// 3. FSkeletalMaterial[] array
        /// 4. FReferenceSkeleton
        /// 5. LOD data (complex, version-dependent)
        /// </summary>
        private void TryParseExtraData()
        {
            try
            {
                // Detect where the material count is located by pattern matching
                bool useLegacyFloats = DetectLegacyBoundsFormat();
                
                using var ms = new MemoryStream(Extras);
                using var extraReader = new AssetBinaryReader(ms, Asset);

                // If we detected the material count offset, skip directly to it
                // This handles cases where the header structure varies
                if (_detectedMaterialCountOffset > 0)
                {
                    // Store the pre-material data as-is (we'll preserve it on write)
                    _preMaterialData = new byte[_detectedMaterialCountOffset];
                    Array.Copy(Extras, 0, _preMaterialData, 0, _detectedMaterialCountOffset);
                    
                    // Read FStripDataFlags from start (for reference)
                    StripFlags = new FStripDataFlags(extraReader);
                    
                    // Skip directly to material count offset
                    extraReader.BaseStream.Position = _detectedMaterialCountOffset;
                    
                    // Create placeholder bounds (we'll preserve original bytes on write)
                    ImportedBounds = new FBoxSphereBounds();
                }
                else
                {
                    // Fallback: try standard parsing
                    StripFlags = new FStripDataFlags(extraReader);
                    ImportedBounds = new FBoxSphereBounds(extraReader);
                }

                // 3. Read FSkeletalMaterial array
                _materialsOffset = (int)extraReader.BaseStream.Position;
                int materialCount = extraReader.ReadInt32();
                
                if (materialCount > 0 && materialCount <= 100)
                {
                    // Try to detect if materials have FGameplayTagContainer by checking structure
                    // Legacy format: 40 bytes per material
                    // With FGameplayTagContainer: 44+ bytes per material (44 for empty container)
                    bool hasGameplayTags = DetectGameplayTagsInMaterials(extraReader, materialCount);
                    _sourceHadGameplayTags = hasGameplayTags;
                    
                    Materials = new List<FSkeletalMaterial>(materialCount);
                    for (int i = 0; i < materialCount; i++)
                    {
                        var mat = new FSkeletalMaterial();
                        mat.Read(extraReader, includeGameplayTags: hasGameplayTags);
                        Materials.Add(mat);
                    }
                    _originalMaterialsByteLength = (int)extraReader.BaseStream.Position - _materialsOffset;
                }
                else
                {
                    // Invalid material count, reset and try pattern matching
                    extraReader.BaseStream.Position = _materialsOffset;
                    TryParseMaterialsByPattern();
                    if (Materials != null && Materials.Count > 0)
                    {
                        // Skip past materials we found
                        extraReader.BaseStream.Position = _materialsOffset + _originalMaterialsByteLength;
                    }
                }

                // 4. Read FReferenceSkeleton
                int skeletonStartPos = (int)extraReader.BaseStream.Position;
                try
                {
                    ReferenceSkeleton = new FReferenceSkeleton(extraReader);
                }
                catch
                {
                    // Failed to parse skeleton, reset position
                    extraReader.BaseStream.Position = skeletonStartPos;
                    ReferenceSkeleton = null;
                }

                // 5. Check for cooked LOD data
                if (extraReader.BaseStream.Position < extraReader.BaseStream.Length - 4)
                {
                    // Try to read bCooked flag and LOD count
                    long posBeforeLOD = extraReader.BaseStream.Position;
                    try
                    {
                        bCooked = extraReader.ReadInt32() != 0;
                        if (bCooked && extraReader.BaseStream.Position < extraReader.BaseStream.Length - 4)
                        {
                            LODCount = extraReader.ReadInt32();
                            if (LODCount < 0 || LODCount > 10)
                            {
                                // Invalid LOD count, probably misread
                                LODCount = 0;
                                extraReader.BaseStream.Position = posBeforeLOD;
                            }
                        }
                    }
                    catch
                    {
                        extraReader.BaseStream.Position = posBeforeLOD;
                    }
                }

                // Store remaining unparsed data
                _parsedDataEndOffset = (int)extraReader.BaseStream.Position;
                int remainingLength = Extras.Length - _parsedDataEndOffset;
                if (remainingLength > 0)
                {
                    RemainingExtraData = new byte[remainingLength];
                    Array.Copy(Extras, _parsedDataEndOffset, RemainingExtraData, 0, remainingLength);
                }
                else
                {
                    RemainingExtraData = Array.Empty<byte>();
                }

                ExtraDataParsed = true;
            }
            catch
            {
                // If structured parsing fails, fall back to pattern matching for materials only
                ExtraDataParsed = false;
                TryParseMaterialsByPattern();
            }
        }

        /// <summary>
        /// Fallback method to find and parse materials by pattern matching.
        /// Used when structured parsing fails. This is a robust pattern-based search
        /// that looks for the FSkeletalMaterial array structure.
        /// Tries both 40-byte (legacy) and 44-byte (with FGameplayTagContainer) formats.
        /// </summary>
        private void TryParseMaterialsByPattern()
        {
            const int MAX_MATERIAL_COUNT = 50;
            
            // Try both material sizes: 40 (legacy) and 44 (with empty FGameplayTagContainer)
            int[] materialSizes = { 40, 44 };
            
            foreach (int materialStructSize in materialSizes)
            {
                bool includeGameplayTags = (materialStructSize == 44);
                
                // Search through the extra data for a valid material array pattern
                for (int i = 4; i < Extras.Length - (materialStructSize * 2); i++)
                {
                    int potentialCount = BitConverter.ToInt32(Extras, i);
                    if (potentialCount < 1 || potentialCount > MAX_MATERIAL_COUNT)
                        continue;
                    
                    // First material's FPackageIndex should be negative (import reference)
                    // Import indices can be quite large negative numbers
                    int firstPkgIdx = BitConverter.ToInt32(Extras, i + 4);
                    if (firstPkgIdx >= 0 || firstPkgIdx < -10000)
                        continue;
                    
                    // Validate the pattern by checking multiple materials with this size
                    bool validPattern = true;
                    int validatedCount = 0;
                    int nameMapCount = Asset?.GetNameMapIndexList()?.Count ?? int.MaxValue;
                    
                    for (int m = 0; m < Math.Min(potentialCount, 5); m++)
                    {
                        int matOffset = i + 4 + (m * materialStructSize);
                        if (matOffset + materialStructSize > Extras.Length)
                        {
                            validPattern = false;
                            break;
                        }
                        
                        // Check FPackageIndex (should be negative for imports, or 0 for null)
                        int pkgIdx = BitConverter.ToInt32(Extras, matOffset);
                        if (pkgIdx > 0 || pkgIdx < -10000)
                        {
                            validPattern = false;
                            break;
                        }
                        
                        // Check FName indices (MaterialSlotName at offset +4)
                        // FName is 8 bytes: 4 bytes index + 4 bytes number
                        int nameIdx = BitConverter.ToInt32(Extras, matOffset + 4);
                        // Allow any non-negative name index - don't validate against name map count
                        // as the name map might not be fully loaded
                        if (nameIdx < 0)
                        {
                            validPattern = false;
                            break;
                        }
                        
                        validatedCount++;
                    }
                    
                    // Need at least 1 validated material (reduced from 2 for single-material meshes)
                    if (!validPattern || validatedCount < 1)
                        continue;
                    
                    // Try to parse materials with this size
                    _materialsOffset = i;
                    int materialCount = potentialCount;
                    
                    Materials = new List<FSkeletalMaterial>();
                    using var ms = new MemoryStream(Extras, i + 4, Extras.Length - i - 4);
                    using var matReader = new AssetBinaryReader(ms, Asset);
                    
                    try
                    {
                        for (int m = 0; m < materialCount; m++)
                        {
                            var mat = new FSkeletalMaterial();
                            mat.Read(matReader, includeGameplayTags: includeGameplayTags);
                            Materials.Add(mat);
                        }
                        
                        _originalMaterialsByteLength = 4 + (materialCount * materialStructSize);
                        _sourceHadGameplayTags = includeGameplayTags;
                        
                        // Successfully parsed materials
                        return;
                    }
                    catch
                    {
                        // If reading fails, clear and continue searching
                        Materials = null;
                        _materialsOffset = -1;
                        _originalMaterialsByteLength = 0;
                    }
                }
            }
        }

        public override void Write(AssetBinaryWriter writer)
        {
            // Ensure extra data is parsed before writing
            EnsureExtraDataParsed();
            
            // If we have materials and need to add FGameplayTagContainer, patch them
            // This MUST happen BEFORE base.Write() so the modified Extras is written by UAsset.WriteData()
            if (Materials != null && Materials.Count > 0 && _materialsOffset >= 0 && IncludeGameplayTags && !_sourceHadGameplayTags)
            {
                ReconstructExtrasWithMaterialsOnly();
            }
            
            base.Write(writer);
        }

        /// <summary>
        /// Reconstruct Extras from fully parsed data.
        /// </summary>
        private void ReconstructExtrasFromParsedData()
        {
            using var ms = new MemoryStream();
            using var extraWriter = new AssetBinaryWriter(ms, Asset);

            // If we have preserved pre-material data, use it directly
            if (_preMaterialData != null && _preMaterialData.Length > 0)
            {
                extraWriter.Write(_preMaterialData);
            }
            else
            {
                // 1. Write FStripDataFlags
                if (StripFlags != null)
                {
                    StripFlags.Write(extraWriter);
                }
                else
                {
                    new FStripDataFlags().Write(extraWriter);
                }

                // 2. Write FBoxSphereBounds
                if (ImportedBounds != null)
                {
                    ImportedBounds.Write(extraWriter);
                }
                else
                {
                    new FBoxSphereBounds().Write(extraWriter);
                }
            }

            // 3. Write FSkeletalMaterial array with FGameplayTagContainer
            extraWriter.Write(Materials.Count);
            foreach (var mat in Materials)
            {
                mat.Write(extraWriter, IncludeGameplayTags);
            }

            // 4. Write FReferenceSkeleton
            if (ReferenceSkeleton != null)
            {
                ReferenceSkeleton.Write(extraWriter);
            }

            // 5. Write bCooked and LOD count if we have them
            if (bCooked || LODCount > 0)
            {
                extraWriter.Write(bCooked ? 1 : 0);
                if (bCooked)
                {
                    extraWriter.Write(LODCount);
                }
            }

            // 6. Write remaining unparsed data
            if (RemainingExtraData != null && RemainingExtraData.Length > 0)
            {
                extraWriter.Write(RemainingExtraData);
            }

            Extras = ms.ToArray();
        }

        /// <summary>
        /// Reconstruct Extras with only materials patched (fallback method).
        /// </summary>
        private void ReconstructExtrasWithMaterialsOnly()
        {
            // Serialize materials to an expandable stream first to get actual byte length.
            // Materials with injected FGameplayTags are larger than the fixed 44-byte empty-tag size
            // (each tag adds 8 bytes for its FName).
            byte[] serializedMaterials;
            using (var matStream = new MemoryStream())
            using (var matWriter = new AssetBinaryWriter(matStream, Asset))
            {
                matWriter.Write(Materials.Count);
                foreach (var mat in Materials)
                {
                    mat.Write(matWriter, IncludeGameplayTags);
                }
                serializedMaterials = matStream.ToArray();
            }
            
            int newMaterialsByteLength = serializedMaterials.Length;
            int sizeDiff = newMaterialsByteLength - _originalMaterialsByteLength;
            
            byte[] newExtras = new byte[Extras.Length + sizeDiff];
            
            Array.Copy(Extras, 0, newExtras, 0, _materialsOffset);
            Array.Copy(serializedMaterials, 0, newExtras, _materialsOffset, newMaterialsByteLength);
            
            int afterMaterialsOffset = _materialsOffset + _originalMaterialsByteLength;
            int afterMaterialsNewOffset = _materialsOffset + newMaterialsByteLength;
            int remainingBytes = Extras.Length - afterMaterialsOffset;
            if (remainingBytes > 0)
            {
                Array.Copy(Extras, afterMaterialsOffset, newExtras, afterMaterialsNewOffset, remainingBytes);
            }
            
            Extras = newExtras;
        }

        #endregion

        #region Helper Methods

        /// <summary>
        /// Detect if FBoxSphereBounds uses legacy (float) or modern (double) format.
        /// Legacy format: FStripDataFlags(2) + FBoxSphereBounds(28 floats) = 30 bytes to material count
        /// Modern format: FStripDataFlags(2) + FBoxSphereBounds(56 doubles) = 58 bytes to material count
        /// </summary>
        private bool DetectLegacyBoundsFormat()
        {
            if (Extras == null || Extras.Length < 40)
            {
                return false;
            }
            
            // Search for material count pattern in first 100 bytes
            // Pattern: valid count (1-50) followed by negative FPackageIndex (-1 to -10000)
            for (int offset = 20; offset < Math.Min(100, Extras.Length - 8); offset++)
            {
                int count = BitConverter.ToInt32(Extras, offset);
                int pkgIdx = BitConverter.ToInt32(Extras, offset + 4);
                
                if (count > 0 && count <= 50 && pkgIdx < 0 && pkgIdx > -10000)
                {
                    // Found material count at this offset
                    // Legacy format: offset around 30 (2 + 28)
                    // Modern format: offset around 58 (2 + 56)
                    bool isLegacy = offset < 45;
                    _detectedMaterialCountOffset = offset;
                    return isLegacy;
                }
            }
            
            // Default to engine version check
            return !(Asset?.ObjectVersionUE5 >= ObjectVersionUE5.LARGE_WORLD_COORDINATES);
        }
        
        private int _detectedMaterialCountOffset = -1;
        private byte[] _preMaterialData = null;

        /// <summary>
        /// Detect if materials in the extra data have FGameplayTagContainer by checking structure.
        /// This is done by reading ahead and checking if the pattern matches 40-byte or 44-byte materials.
        /// </summary>
        private bool DetectGameplayTagsInMaterials(AssetBinaryReader reader, int materialCount)
        {
            if (materialCount < 2) return false; // Need at least 2 materials to detect pattern
            
            long startPos = reader.BaseStream.Position;
            
            try
            {
                // Read first material without tags (40 bytes)
                // FPackageIndex (4) + FName (8) + FName (8) + FMeshUVChannelInfo (20) = 40 bytes
                int firstPkgIdx = reader.ReadInt32(); // MaterialInterface
                reader.ReadFName(); // MaterialSlotName
                reader.ReadFName(); // ImportedMaterialSlotName
                reader.BaseStream.Position += 20; // FMeshUVChannelInfo
                
                // Now check what comes next
                // If it's FGameplayTagContainer, the next int32 should be a small count (0-10)
                // followed by tag FNames, then the next material's FPackageIndex
                
                if (reader.BaseStream.Position + 8 > reader.BaseStream.Length)
                {
                    reader.BaseStream.Position = startPos;
                    return false;
                }
                
                int potentialTagCount = reader.ReadInt32();
                
                // If potentialTagCount is 0, skip it and check the next value
                // If it's a valid FPackageIndex (negative), then we have FGameplayTagContainer with 0 tags
                if (potentialTagCount == 0)
                {
                    int nextValue = reader.ReadInt32();
                    reader.BaseStream.Position = startPos;
                    
                    // If next value is negative (FPackageIndex for next material), we have empty FGameplayTagContainer
                    if (nextValue < 0 && nextValue > -1000)
                    {
                        return true;
                    }
                    return false;
                }
                
                // If potentialTagCount is negative, it's likely the next material's FPackageIndex (no tags)
                if (potentialTagCount < 0)
                {
                    reader.BaseStream.Position = startPos;
                    return false;
                }
                
                // If potentialTagCount is a small positive number (1-10), it might be tag count
                // Skip the tags and check if the next value is a valid FPackageIndex
                if (potentialTagCount > 0 && potentialTagCount <= 10)
                {
                    // Skip tag FNames (8 bytes each)
                    reader.BaseStream.Position += potentialTagCount * 8;
                    
                    if (reader.BaseStream.Position + 4 > reader.BaseStream.Length)
                    {
                        reader.BaseStream.Position = startPos;
                        return false;
                    }
                    
                    int nextPkgIdx = reader.ReadInt32();
                    reader.BaseStream.Position = startPos;
                    
                    // If next value is a valid FPackageIndex (negative), we have FGameplayTagContainer
                    if (nextPkgIdx < 0 && nextPkgIdx > -1000)
                    {
                        return true;
                    }
                }
                
                reader.BaseStream.Position = startPos;
                return false;
            }
            catch
            {
                reader.BaseStream.Position = startPos;
                return false;
            }
        }

        /// <summary>
        /// Get the number of bones in the skeleton.
        /// </summary>
        public int GetBoneCount()
        {
            return ReferenceSkeleton?.BoneCount ?? 0;
        }

        /// <summary>
        /// Get bone info by index.
        /// </summary>
        public FMeshBoneInfo GetBone(int index)
        {
            return ReferenceSkeleton?.GetBoneInfo(index);
        }

        /// <summary>
        /// Get bone transform by index.
        /// </summary>
        public FTransform? GetBoneTransform(int index)
        {
            return ReferenceSkeleton?.GetBonePose(index);
        }

        /// <summary>
        /// Find bone index by name.
        /// </summary>
        public int FindBoneIndex(FName boneName)
        {
            return ReferenceSkeleton?.FindBoneIndex(boneName) ?? -1;
        }

        /// <summary>
        /// Get material by index.
        /// </summary>
        public FSkeletalMaterial GetMaterial(int index)
        {
            if (Materials != null && index >= 0 && index < Materials.Count)
            {
                return Materials[index];
            }
            return null;
        }

        /// <summary>
        /// Get the number of materials.
        /// </summary>
        public int GetMaterialCount()
        {
            return Materials?.Count ?? 0;
        }

        #endregion
    }
}
