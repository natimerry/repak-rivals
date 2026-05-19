using System;
using System.Collections.Generic;
using System.IO;
using UAssetAPI.PropertyTypes.Objects;
using UAssetAPI.UnrealTypes;
using UAssetAPI.Unversioned;

namespace UAssetAPI.ExportTypes.Texture
{
    /// <summary>
    /// Export type for UTexture2D and related texture classes.
    /// Properly parses FTexturePlatformData instead of storing it in Extras.
    /// Based on CUE4Parse's UTexture2D implementation.
    /// </summary>
    public class TextureExport : NormalExport
    {
        /// <summary>
        /// The parsed texture platform data containing mipmaps.
        /// </summary>
        public FTexturePlatformData PlatformData;

        /// <summary>
        /// Path to external .ubulk file if present.
        /// </summary>
        public string BulkFilePath;

        /// <summary>
        /// Whether this is a 2D texture (vs cube map, volume, etc.)
        /// </summary>
        public bool IsTexture2D = true;

        /// <summary>
        /// Strip data flags read from the texture.
        /// </summary>
        public byte StripDataFlags1;
        public byte StripDataFlags2;

        /// <summary>
        /// Whether the texture is cooked.
        /// </summary>
        public bool bCooked;

        /// <summary>
        /// Whether to serialize mip data (UE5.3+).
        /// </summary>
        public bool bSerializeMipData = true;

        /// <summary>
        /// Pixel format name ID (raw uint64, not FName).
        /// </summary>
        public ulong PixelFormatNameId;

        /// <summary>
        /// Skip offset for cooked platform data.
        /// </summary>
        public long SkipOffset;

        /// <summary>
        /// Position where skip offset was written (for updating on write).
        /// </summary>
        private long _skipOffsetPosition;

        /// <summary>
        /// Strip data flags 3 (UTexture2D GlobalStripFlags).
        /// </summary>
        public byte StripDataFlags3;

        /// <summary>
        /// Strip data flags 4 (UTexture2D ClassStripFlags).
        /// </summary>
        public byte StripDataFlags4;

        /// <summary>
        /// Unknown uint32 for UE4.20+.
        /// </summary>
        public uint UnknownUint32;

        /// <summary>
        /// Placeholder bytes for UE5.0+ (16 bytes).
        /// </summary>
        public byte[] PlaceholderBytes;

        /// <summary>
        /// LightingGuid bytes (16 bytes) serialized after properties in Marvel Rivals textures.
        /// </summary>
        public byte[] LightingGuidBytes;

        /// <summary>
        /// Pixel format FName for round-trip serialization.
        /// </summary>
        public FName PixelFormatFName;
        
        /// <summary>
        /// None FName read at the end of pixel format loop (for round-trip serialization).
        /// </summary>
        public FName NoneFName;
        
        /// <summary>
        /// Extra bytes between PixelFormat FName and skip offset (Marvel Rivals specific).
        /// </summary>
        public byte[] ExtraBytes;

        public TextureExport(Export super) : base(super)
        {
        }

        public TextureExport(UAsset asset, byte[] extras) : base(asset, extras)
        {
        }

        public TextureExport() : base()
        {
        }

        public override void Read(AssetBinaryReader reader, int nextStarting = 0)
        {
            // For versioned properties, use base.Read which handles ObjectGuid correctly
            // For unversioned properties (Marvel Rivals), use custom ReadTextureProperties
            if (reader.Asset.HasUnversionedProperties)
            {
                // Read properties WITHOUT ObjectGuid (textures don't have it in unversioned format)
                ReadTextureProperties(reader);
            }
            else
            {
                // Use base.Read for versioned properties - it reads ObjectGuid
                // For textures, ObjectGuid is actually the LightingGuid
                base.Read(reader, nextStarting);
                
                // Store the ObjectGuid as LightingGuid
                if (ObjectGuid.HasValue)
                {
                    LightingGuidBytes = ObjectGuid.Value.ToByteArray();
                }
                
                // Set bulk file path for reading external mip data
                if (Asset is UAsset uassetVersioned && !string.IsNullOrEmpty(uassetVersioned.FilePath))
                {
                    BulkFilePath = Path.ChangeExtension(uassetVersioned.FilePath, ".ubulk");
                }
                
                // Read remaining bytes as texture data (base.Read doesn't populate Extras)
                long versionedRemainingBytes = nextStarting - reader.BaseStream.Position;
                
                if (versionedRemainingBytes > 0)
                {
                    byte[] textureData = reader.ReadBytes((int)versionedRemainingBytes);
                    
                    // Parse texture data (without LightingGuid since it's in ObjectGuid)
                    using (var ms = new MemoryStream(textureData))
                    using (var extraReader = new AssetBinaryReader(ms, reader.Asset))
                    {
                        ParseTextureDataVersioned(extraReader, textureData.Length);
                    }
                    
                    // Store in Extras for fallback if parsing failed
                    if (PlatformData == null)
                    {
                        Extras = textureData;
                    }
                }
                return;
            }

            // Determine bulk file path
            if (Asset is UAsset uasset && !string.IsNullOrEmpty(uasset.FilePath))
            {
                BulkFilePath = Path.ChangeExtension(uasset.FilePath, ".ubulk");
            }

            long remainingBytes = nextStarting - reader.BaseStream.Position;
            if (remainingBytes <= 0)
            {
                return;
            }

            // Texture parsing for UE5.3+ (Marvel Rivals format)
            // Based on CUE4Parse's UTexture2D implementation
            // Structure: UTexture.Deserialize -> FStripDataFlags -> UTexture2D.Deserialize -> FStripDataFlags -> bCooked -> bSerializeMipData
            //            -> DeserializeCookedPlatformData -> PixelFormat FName -> SkipOffset -> FTexturePlatformData
            bool enableTextureParsing = true;
            if (!enableTextureParsing)
            {
                Extras = reader.ReadBytes((int)remainingBytes);
                return;
            }

            try
            {
                long startPos = reader.BaseStream.Position;
                
                // Check if LightingGuid was already read as a property
                // If so, don't read it again as raw bytes
                bool hasLightingGuidProperty = false;
                if (Data != null)
                {
                    foreach (var prop in Data)
                    {
                        if (prop.Name.Value?.Value == "LightingGuid")
                        {
                            hasLightingGuidProperty = true;
                            LightingGuidBytes = new byte[16]; // Placeholder, actual value is in property
                            break;
                        }
                    }
                }
                
                // Only read LightingGuid as raw bytes if it wasn't in properties
                if (!hasLightingGuidProperty)
                {
                    LightingGuidBytes = reader.ReadBytes(16);
                }
                
                // UTexture::Deserialize reads FStripDataFlags (2 bytes)
                // UTexture2D::Deserialize reads another FStripDataFlags (2 bytes)
                // Total: 4 bytes of strip flags
                StripDataFlags1 = reader.ReadByte(); // UTexture GlobalStripFlags
                StripDataFlags2 = reader.ReadByte(); // UTexture ClassStripFlags
                StripDataFlags3 = reader.ReadByte(); // UTexture2D GlobalStripFlags
                StripDataFlags4 = reader.ReadByte(); // UTexture2D ClassStripFlags

                // bCooked (int32 as bool) - Marvel Rivals uses 0x00010001 for true
                int bCookedRaw = reader.ReadInt32();
                bCooked = bCookedRaw != 0;

                if (bCooked)
                {
                    // UE5.3+: bSerializeMipData (int32 as bool)
                    bSerializeMipData = reader.ReadInt32() != 0;

                    // DeserializeCookedPlatformData (CUE4Parse approach)
                    // Read pixel format name as FName
                    var pixelFormatName = reader.ReadFName();
                    PixelFormatFName = pixelFormatName; // Store for round-trip serialization
                    string pixelFormatStr = pixelFormatName?.Value?.Value ?? "null";
                    
                    // Marvel Rivals has an extra 4 bytes (unknown purpose) before skip offset
                    // Check if next 4 bytes are zeros followed by a reasonable skip offset
                    long checkPos = reader.BaseStream.Position;
                    int potentialExtra = reader.ReadInt32();
                    long potentialSkipOffset = reader.ReadInt64();
                    reader.BaseStream.Position = checkPos;
                    
                    // If the first 4 bytes are 0 and the next 8 bytes give a reasonable skip offset, skip the extra bytes
                    if (potentialExtra == 0 && potentialSkipOffset > 0 && potentialSkipOffset < 100000)
                    {
                        ExtraBytes = reader.ReadBytes(4); // Store for round-trip
                    }
                    
                    // Loop while pixelFormatName is not None (CUE4Parse reads multiple formats)
                    while (pixelFormatName != null && pixelFormatName.Value?.Value != "None")
                    {
                        // Skip offset (int64 for UE5.0+) - relative from AFTER reading the offset
                        _skipOffsetPosition = reader.BaseStream.Position;
                        long skipOffsetRel = reader.ReadInt64();
                        SkipOffset = reader.BaseStream.Position + skipOffsetRel;
                        
                        // Validate skip offset is within bounds
                        if (SkipOffset < 0 || SkipOffset > reader.BaseStream.Length)
                        {
                            Console.Error.WriteLine($"[TextureExport] Invalid skip offset: {SkipOffset} (stream length: {reader.BaseStream.Length})");
                            SkipOffset = nextStarting; // Use export end as fallback
                        }

                        // Try to read FTexturePlatformData
                        try
                        {
                            PlatformData = new FTexturePlatformData(reader, BulkFilePath, bSerializeMipData, true);
                        }
                        catch (Exception ex)
                        {
                            Console.Error.WriteLine($"[TextureExport] FTexturePlatformData parsing failed: {ex.Message}");
                            PlatformData = null;
                        }

                        // Move to skip offset position (or end of export if invalid)
                        long targetPos = Math.Min(SkipOffset, reader.BaseStream.Length);
                        if (reader.BaseStream.Position != targetPos && targetPos >= 0)
                        {
                            reader.BaseStream.Position = targetPos;
                        }

                        // Read next pixel format name
                        if (reader.BaseStream.Position < nextStarting - 8)
                        {
                            pixelFormatName = reader.ReadFName();
                            // Store the None FName for round-trip serialization
                            if (pixelFormatName != null && pixelFormatName.Value?.Value == "None")
                            {
                                NoneFName = pixelFormatName;
                            }
                        }
                        else
                        {
                            break;
                        }
                    }

                    // If parsing failed, store remaining data in Extras for fallback
                    if (PlatformData == null || PlatformData.Mips == null || PlatformData.Mips.Count == 0)
                    {
                        reader.BaseStream.Position = startPos;
                        Extras = reader.ReadBytes((int)remainingBytes);
                    }
                    else
                    {
                        Extras = Array.Empty<byte>();
                    }
                }
                else
                {
                    // Not cooked - read remaining as Extras
                    reader.BaseStream.Position = startPos;
                    Extras = reader.ReadBytes((int)remainingBytes);
                }
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"[TextureExport] Failed to parse texture data: {ex.Message}");
                Console.Error.WriteLine($"[TextureExport] Stack: {ex.StackTrace}");
                
                // On failure, try to read remaining as Extras
                long remaining = nextStarting - reader.BaseStream.Position;
                if (remaining > 0)
                {
                    Extras = reader.ReadBytes((int)remaining);
                }
            }
        }

        /// <summary>
        /// Parse texture data from Extras (for versioned properties format).
        /// The structure is the same as unversioned, just stored in Extras after base.Read().
        /// </summary>
        private void ParseTextureData(AssetBinaryReader reader, int dataLength)
        {
            try
            {
                // Structure for versioned properties textures:
                // - LightingGuid (16 bytes)
                // - StripDataFlags (4 bytes)
                // - bCooked (4 bytes)
                // - bSerializeMipData (4 bytes)
                // - PixelFormat FName (8 bytes)
                // - Skip offset (8 bytes)
                // - FTexturePlatformData
                
                LightingGuidBytes = reader.ReadBytes(16);
                
                StripDataFlags1 = reader.ReadByte();
                StripDataFlags2 = reader.ReadByte();
                StripDataFlags3 = reader.ReadByte();
                StripDataFlags4 = reader.ReadByte();
                
                int bCookedRaw = reader.ReadInt32();
                bCooked = bCookedRaw != 0;
                
                if (!bCooked)
                {
                    Console.Error.WriteLine("[TextureExport] Texture is not cooked, skipping platform data parsing");
                    return;
                }
                
                bSerializeMipData = reader.ReadInt32() != 0;
                
                // Read pixel format FName
                PixelFormatFName = reader.ReadFName();
                
                // Check for extra bytes before skip offset
                long checkPos = reader.BaseStream.Position;
                int potentialExtra = reader.ReadInt32();
                long potentialSkipOffset = reader.ReadInt64();
                reader.BaseStream.Position = checkPos;
                
                if (potentialExtra == 0 && potentialSkipOffset > 0 && potentialSkipOffset < 100000)
                {
                    ExtraBytes = reader.ReadBytes(4);
                }
                
                // Read skip offset
                _skipOffsetPosition = reader.BaseStream.Position;
                long skipOffsetRel = reader.ReadInt64();
                SkipOffset = reader.BaseStream.Position + skipOffsetRel;
                
                // Parse FTexturePlatformData
                PlatformData = new FTexturePlatformData(reader, BulkFilePath, bSerializeMipData, true);
                
                // Read None FName at end if present
                if (reader.BaseStream.Position < dataLength - 8)
                {
                    var noneName = reader.ReadFName();
                    if (noneName?.Value?.Value == "None")
                    {
                        NoneFName = noneName;
                    }
                }
                
            }
            catch (Exception)
            {
                PlatformData = null;
            }
        }

        /// <summary>
        /// Parse texture data from Extras for versioned properties format.
        /// LightingGuid is already in ObjectGuid, so this starts with StripFlags.
        /// </summary>
        private void ParseTextureDataVersioned(AssetBinaryReader reader, int dataLength)
        {
            try
            {
                // Structure for versioned properties textures (LightingGuid already read as ObjectGuid):
                // - StripDataFlags (4 bytes)
                // - bCooked (4 bytes)
                // - bSerializeMipData (4 bytes)
                // - PixelFormat FName (8 bytes)
                // - Skip offset (8 bytes)
                // - FTexturePlatformData
                // - None FName (8 bytes)
                
                StripDataFlags1 = reader.ReadByte();
                StripDataFlags2 = reader.ReadByte();
                StripDataFlags3 = reader.ReadByte();
                StripDataFlags4 = reader.ReadByte();
                
                int bCookedRaw = reader.ReadInt32();
                bCooked = bCookedRaw != 0;
                
                if (!bCooked)
                {
                    return;
                }
                
                bSerializeMipData = reader.ReadInt32() != 0;
                
                // Read pixel format FName
                PixelFormatFName = reader.ReadFName();
                
                // Check for extra bytes before skip offset (Marvel Rivals specific)
                long checkPos = reader.BaseStream.Position;
                int potentialExtra = reader.ReadInt32();
                long potentialSkipOffset = reader.ReadInt64();
                reader.BaseStream.Position = checkPos;
                
                if (potentialExtra == 0 && potentialSkipOffset > 0 && potentialSkipOffset < 100000)
                {
                    ExtraBytes = reader.ReadBytes(4);
                }
                
                // Read skip offset
                _skipOffsetPosition = reader.BaseStream.Position;
                long skipOffsetRel = reader.ReadInt64();
                SkipOffset = reader.BaseStream.Position + skipOffsetRel;
                
                // Parse FTexturePlatformData
                PlatformData = new FTexturePlatformData(reader, BulkFilePath, bSerializeMipData, true);
                
                // Read None FName at end if present
                if (reader.BaseStream.Position < dataLength - 8)
                {
                    var noneName = reader.ReadFName();
                    if (noneName?.Value?.Value == "None")
                    {
                        NoneFName = noneName;
                    }
                }
                
            }
            catch (Exception)
            {
                PlatformData = null;
            }
        }

        /// <summary>
        /// Write texture data to a stream (for versioned properties format).
        /// LightingGuid is written as ObjectGuid by base.Write(), so we start with StripFlags.
        /// </summary>
        private void WriteTextureData(AssetBinaryWriter writer)
        {
            // NOTE: LightingGuid is written as ObjectGuid by base.Write(), not here
            
            // Write strip data flags
            writer.Write(StripDataFlags1);
            writer.Write(StripDataFlags2);
            writer.Write(StripDataFlags3);
            writer.Write(StripDataFlags4);
            
            // Write bCooked - use standard format (1 or 0), not Marvel Rivals format (0x00010001)
            writer.Write(bCooked ? 1 : 0);
            
            if (bCooked)
            {
                // Write bSerializeMipData
                writer.Write(bSerializeMipData ? 1 : 0);
                
                // Write pixel format FName
                if (PixelFormatFName != null)
                {
                    writer.Write(PixelFormatFName);
                }
                else if (!string.IsNullOrEmpty(PlatformData?.PixelFormat))
                {
                    writer.Write(FName.FromString(Asset, PlatformData.PixelFormat));
                }
                else
                {
                    writer.Write(FName.FromString(Asset, "None"));
                }
                
                // Write extra bytes if present
                if (ExtraBytes != null && ExtraBytes.Length > 0)
                {
                    writer.Write(ExtraBytes);
                }
                
                // Write skip offset placeholder
                long skipOffsetPos = writer.BaseStream.Position;
                writer.Write((long)0);
                
                // Write FTexturePlatformData
                PlatformData.Write(writer);
                
                // Update skip offset
                long currentPos = writer.BaseStream.Position;
                writer.BaseStream.Position = skipOffsetPos;
                writer.Write((long)(currentPos - skipOffsetPos - 8));
                writer.BaseStream.Position = currentPos;
                
                // Write None FName
                if (NoneFName != null)
                {
                    writer.Write(NoneFName);
                }
                else
                {
                    writer.Write(FName.FromString(Asset, "None"));
                }
            }
        }

        public override void Write(AssetBinaryWriter writer)
        {
            // For versioned properties, use base.Write and put texture data in Extras
            // For unversioned properties (Marvel Rivals), use custom WriteTextureProperties
            if (!writer.Asset.HasUnversionedProperties)
            {
                // Versioned properties - write texture data to Extras, then use base.Write
                if (PlatformData != null)
                {
                    using (var ms = new MemoryStream())
                    using (var extraWriter = new AssetBinaryWriter(ms, writer.Asset))
                    {
                        WriteTextureData(extraWriter);
                        Extras = ms.ToArray();
                    }
                }
                
                // Set ObjectGuid from LightingGuidBytes so base.Write() writes it correctly
                if (LightingGuidBytes != null && LightingGuidBytes.Length == 16)
                {
                    ObjectGuid = new Guid(LightingGuidBytes);
                }
                
                base.Write(writer);
                return;
            }

            // Unversioned properties - write properties WITHOUT ObjectGuid
            WriteTextureProperties(writer);

            // If we have parsed platform data, write the full texture structure
            if (PlatformData != null)
            {
                // Check if LightingGuid is already in properties - if so, don't write it again
                bool hasLightingGuidProperty = false;
                if (Data != null)
                {
                    foreach (var prop in Data)
                    {
                        if (prop.Name.Value?.Value == "LightingGuid")
                        {
                            hasLightingGuidProperty = true;
                            break;
                        }
                    }
                }
                
                // Only write LightingGuid as raw bytes if it's not in properties
                if (!hasLightingGuidProperty)
                {
                    writer.Write(LightingGuidBytes ?? new byte[16]);
                }
                
                // Write strip data flags (4 bytes total: 2 from UTexture + 2 from UTexture2D)
                writer.Write(StripDataFlags1);
                writer.Write(StripDataFlags2);
                writer.Write(StripDataFlags3);
                writer.Write(StripDataFlags4);

                // Write bCooked - Marvel Rivals uses 0x00010001 for true
                writer.Write(bCooked ? 0x00010001 : 0);

                if (bCooked)
                {
                    // bSerializeMipData (int32 as bool)
                    writer.Write(bSerializeMipData ? 1 : 0);

                    // Write pixel format name as FName (same as Read)
                    // Use the stored PixelFormatFName or create one from PlatformData
                    if (PixelFormatFName != null)
                    {
                        writer.Write(PixelFormatFName);
                    }
                    else if (!string.IsNullOrEmpty(PlatformData.PixelFormat))
                    {
                        // Create FName from pixel format string
                        writer.Write(FName.FromString(Asset, PlatformData.PixelFormat));
                    }
                    else
                    {
                        // Fallback - write "None"
                        writer.Write(FName.FromString(Asset, "None"));
                    }

                    // Write extra bytes if present (Marvel Rivals specific)
                    if (ExtraBytes != null && ExtraBytes.Length > 0)
                    {
                        writer.Write(ExtraBytes);
                    }
                    
                    // Write skip offset placeholder - we'll update it later
                    // UE5.0+: skip offset is int64 (8 bytes)
                    long skipOffsetPos = writer.BaseStream.Position;
                    writer.Write((long)0); // Placeholder for skip offset (int64 for UE5)

                    // Write FTexturePlatformData
                    PlatformData.Write(writer);

                    // Update skip offset (int64 for UE5)
                    // The skip offset is relative from AFTER reading the offset field
                    long currentPos = writer.BaseStream.Position;
                    writer.BaseStream.Position = skipOffsetPos;
                    writer.Write((long)(currentPos - skipOffsetPos - 8));
                    writer.BaseStream.Position = currentPos;
                    
                    // Write "None" FName to terminate the pixel format loop
                    // Use the stored NoneFName if available for round-trip serialization
                    if (NoneFName != null)
                    {
                        writer.Write(NoneFName);
                    }
                    else
                    {
                        writer.Write(FName.FromString(Asset, "None"));
                    }
                }
            }
            else if (Extras != null && Extras.Length > 0)
            {
                // Fall back to writing Extras if we don't have parsed platform data
                writer.Write(Extras);
            }
        }

        /// <summary>
        /// Strip all mipmaps except the first one and convert to inline storage.
        /// This eliminates the need for .ubulk files.
        /// </summary>
        /// <returns>True if mipmaps were stripped, false if not a texture or already had 1 mipmap.</returns>
        public bool StripMipmaps()
        {
            if (PlatformData == null)
            {
                return false;
            }

            return PlatformData.StripMipmaps();
        }

        /// <summary>
        /// Get the number of mipmaps in this texture.
        /// </summary>
        public int MipCount => PlatformData?.Mips?.Count ?? 0;

        /// <summary>
        /// Get the pixel format of this texture.
        /// </summary>
        public string PixelFormat => PlatformData?.PixelFormat ?? string.Empty;

        /// <summary>
        /// Get the texture dimensions.
        /// </summary>
        public (int Width, int Height) Dimensions => (PlatformData?.SizeX ?? 0, PlatformData?.SizeY ?? 0);

        /// <summary>
        /// Check if this texture has external bulk data (.ubulk file).
        /// </summary>
        public bool HasExternalBulkData
        {
            get
            {
                if (PlatformData?.Mips == null) return false;
                foreach (var mip in PlatformData.Mips)
                {
                    if (mip.BulkData?.Header?.IsInSeparateFile == true)
                    {
                        return true;
                    }
                }
                return false;
            }
        }

        /// <summary>
        /// Read texture properties without the ObjectGuid boolean that NormalExport reads.
        /// Textures don't have ObjectGuid - the texture data starts immediately after properties.
        /// </summary>
        private void ReadTextureProperties(AssetBinaryReader reader)
        {
            // 5.4-specific problem
            if (reader.Asset.ObjectVersionUE5 > ObjectVersionUE5.DATA_RESOURCES && 
                reader.Asset.ObjectVersionUE5 < ObjectVersionUE5.ASSETREGISTRY_PACKAGEBUILDDEPENDENCIES && 
                !ObjectFlags.HasFlag(EObjectFlags.RF_ClassDefaultObject))
            {
                int dummy = reader.ReadInt32();
                if (dummy != 0) throw new FormatException("Expected 4 null bytes at start of NormalExport; got " + dummy);
            }

            Data = new List<PropertyData>();
            PropertyData bit;

            var unversionedHeader = new FUnversionedHeader(reader);
            if (!reader.Asset.HasUnversionedProperties && reader.Asset.ObjectVersionUE5 >= ObjectVersionUE5.PROPERTY_TAG_EXTENSION_AND_OVERRIDABLE_SERIALIZATION)
            {
                SerializationControl = (EClassSerializationControlExtension)reader.ReadByte();

                if (SerializationControl.HasFlag(EClassSerializationControlExtension.OverridableSerializationInformation))
                {
                    Operation = (EOverriddenPropertyOperation)reader.ReadByte();
                }
            }
            FName parentName = GetClassTypeForAncestry(reader.Asset, out FName parentModulePath);
            while ((bit = MainSerializer.Read(reader, null, parentName, parentModulePath, unversionedHeader, true)) != null)
            {
                Data.Add(bit);
            }

            // NOTE: We intentionally skip reading ObjectGuid here because textures don't have it.
        }

        /// <summary>
        /// Write texture properties without the ObjectGuid that NormalExport writes.
        /// Textures don't have ObjectGuid - the texture data starts immediately after properties.
        /// </summary>
        private void WriteTextureProperties(AssetBinaryWriter writer)
        {
            // 5.4-specific problem
            if (writer.Asset.ObjectVersionUE5 > ObjectVersionUE5.DATA_RESOURCES && 
                writer.Asset.ObjectVersionUE5 < ObjectVersionUE5.ASSETREGISTRY_PACKAGEBUILDDEPENDENCIES && 
                !ObjectFlags.HasFlag(EObjectFlags.RF_ClassDefaultObject))
            {
                writer.Write((int)0);
            }

            FName parentName = GetClassTypeForAncestry(writer.Asset, out FName parentModulePath);

            if (OriginalUnversionedHeader != null && writer.Asset.HasUnversionedProperties)
            {
                OriginalUnversionedHeader.Write(writer);
            }
            else
            {
                MainSerializer.GenerateUnversionedHeader(ref Data, parentName, parentModulePath, writer.Asset)?.Write(writer);
            }

            if (!writer.Asset.HasUnversionedProperties && writer.Asset.ObjectVersionUE5 >= ObjectVersionUE5.PROPERTY_TAG_EXTENSION_AND_OVERRIDABLE_SERIALIZATION)
            {
                writer.Write((byte)SerializationControl);

                if (SerializationControl.HasFlag(EClassSerializationControlExtension.OverridableSerializationInformation))
                {
                    writer.Write((byte)Operation);
                }
            }

            for (int j = 0; j < Data.Count; j++)
            {
                PropertyData current = Data[j];
                MainSerializer.Write(current, writer, true);
            }
            if (!writer.Asset.HasUnversionedProperties) writer.Write(new FName(writer.Asset, "None"));

            // NOTE: We intentionally skip writing ObjectGuid here because textures don't have it.
        }
    }
}
