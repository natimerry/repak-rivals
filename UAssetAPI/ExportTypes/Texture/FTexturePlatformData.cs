using System;
using System.Collections.Generic;
using UAssetAPI.UnrealTypes;

namespace UAssetAPI.ExportTypes.Texture
{
    /// <summary>
    /// Texture platform data containing mipmaps and format information.
    /// Ported from CUE4Parse with write support added.
    /// </summary>
    public class FTexturePlatformData
    {
        private const uint BitMask_CubeMap = 1u << 31;
        private const uint BitMask_HasOptData = 1u << 30;
        private const uint BitMask_HasCpuCopy = 1u << 29;
        private const uint BitMask_NumSlices = BitMask_HasOptData - 1u;

        public int SizeX;
        public int SizeY;
        public uint PackedData;
        public string PixelFormat;
        public FOptTexturePlatformData OptData;
        public int FirstMipToSerialize;
        public List<FTexture2DMipMap> Mips;

        /// <summary>
        /// Path to external .ubulk file for loading mipmap data.
        /// </summary>
        public string BulkFilePath;

        /// <summary>
        /// Placeholder bytes for UE5.0+ cooked assets (16 bytes).
        /// </summary>
        public byte[] PlaceholderBytes;

        public FTexturePlatformData()
        {
            SizeX = 0;
            SizeY = 0;
            PackedData = 1; // Default to 1 slice
            PixelFormat = string.Empty;
            OptData = new FOptTexturePlatformData();
            FirstMipToSerialize = 0;
            Mips = new List<FTexture2DMipMap>();
        }

        public FTexturePlatformData(AssetBinaryReader reader, string bulkFilePath = null, bool bSerializeMipData = true, bool isUE5Cooked = false)
        {
            BulkFilePath = bulkFilePath;
            Read(reader, bSerializeMipData, isUE5Cooked);
        }

        public void Read(AssetBinaryReader reader, bool bSerializeMipData = true, bool isUE5Cooked = false)
        {
            // FTexturePlatformData::SerializeCooked — CUE4Parse approach
            // UE5.2+: 1 byte bUsingDerivedData flag + 15 bytes placeholder = 16 total
            // UE5.0-5.1 (IsFilterEditorOnly): 16 bytes flat
            PlaceholderByteCount = 0;
            if (isUE5Cooked)
            {
                const int PlaceholderDerivedDataSize = 16;
                PlaceholderByteCount = PlaceholderDerivedDataSize;
                PlaceholderBytes = reader.ReadBytes(PlaceholderDerivedDataSize);
            }
            
            // Read dimensions and packed data
            SizeX = reader.ReadInt32();
            SizeY = reader.ReadInt32();
            PackedData = reader.ReadUInt32();

            // Read pixel format as FString
            PixelFormat = reader.ReadFString()?.Value ?? string.Empty;

            // Optional texture platform data (if HasOptData flag is set in PackedData)
            if (HasOptData())
            {
                OptData = new FOptTexturePlatformData();
                OptData.ExtData = reader.ReadUInt32();
                OptData.NumMipsInTail = reader.ReadUInt32();
            }

            // First mip to serialize (cooked assets only)
            FirstMipToSerialize = reader.ReadInt32();

            // Read mipmap count and mipmaps
            int mipCount = reader.ReadInt32();
            
            if (mipCount < 0 || mipCount > 20)
            {
                throw new InvalidOperationException($"Invalid mip count: {mipCount}. Parsing error at position {reader.BaseStream.Position}");
            }
            
            Mips = new List<FTexture2DMipMap>(mipCount);

            for (int i = 0; i < mipCount; i++)
            {
                var mip = new FTexture2DMipMap(reader, BulkFilePath, bSerializeMipData);
                Mips.Add(mip);
            }

            // Check if using UE5.3+ DataResources format
            bool hasDataResources = Mips.Count > 0 && Mips[0].BulkData?.Header?.DataResourceIndex >= 0;

            if (hasDataResources)
            {
                // DataResources format: each mip header is just a 4-byte DataResourceIndex.
                // The pixel data is at DataResource.SerialOffset in .uexp (or .ubulk),
                // NOT inline in the stream after the headers.
                // Populate mip dimensions from the header SizeX/SizeY, scaled per mip level.
                for (int mi = 0; mi < Mips.Count; mi++)
                {
                    Mips[mi].SizeX = Math.Max(1, SizeX >> mi);
                    Mips[mi].SizeY = Math.Max(1, SizeY >> mi);
                    Mips[mi].SizeZ = 1;
                }
            }

            // bIsVirtual (int32 as bool) comes right after mip headers
            bIsVirtual = reader.ReadInt32() != 0;

            // Update dimensions from first mip if available (CUE4Parse does this)
            if (Mips.Count > 0)
            {
                SizeX = Mips[0].SizeX;
                SizeY = Mips[0].SizeY;
            }
        }
        
        /// <summary>
        /// Whether this is a virtual texture (UE4.23+).
        /// </summary>
        public bool bIsVirtual;

        /// <summary>
        /// Number of placeholder bytes to write before the actual data (for UE5 cooked assets).
        /// CUE4Parse uses 16 bytes for UE5.0+.
        /// </summary>
        public int PlaceholderByteCount = 16;

        public void Write(AssetBinaryWriter writer)
        {
            // Write placeholder bytes for UE5.0+ cooked assets (16 bytes)
            if (PlaceholderBytes != null && PlaceholderBytes.Length > 0)
            {
                writer.Write(PlaceholderBytes);
            }
            else if (PlaceholderByteCount > 0)
            {
                writer.Write(new byte[PlaceholderByteCount]);
            }
            
            // Write dimensions and packed data
            int writeX = Mips.Count > 0 ? Mips[0].SizeX : SizeX;
            int writeY = Mips.Count > 0 ? Mips[0].SizeY : SizeY;
            writer.Write(writeX);
            writer.Write(writeY);
            writer.Write(PackedData);

            // Write pixel format
            writer.Write(new FString(PixelFormat));

            // Optional texture platform data
            if (HasOptData())
            {
                writer.Write(OptData.ExtData);
                writer.Write(OptData.NumMipsInTail);
            }

            // First mip to serialize
            writer.Write(FirstMipToSerialize);

            // Write mipmap count and mipmaps (headers only for UE5.3+ with DataResources)
            writer.Write(Mips.Count);
            foreach (var mip in Mips)
            {
                mip.Write(writer);
            }

            // Check if using UE5.3+ DataResource format
            bool hasDataResources = Mips.Count > 0 && Mips[0].BulkData?.Header?.DataResourceIndex >= 0;

            if (!hasDataResources)
            {
                // Legacy format: bIsVirtual comes after mip headers, before pixel data
                writer.Write(bIsVirtual ? 1 : 0);
            }

            // Write mip pixel data after all headers
            // For UE5.3+ with DataResources, the DataResource's SerialOffset points to this location
            // For legacy format, inline data is written by FByteBulkData.Write()
            foreach (var mip in Mips)
            {
                // Write pixel data if it's inline (either via DataResourceIndex or IsInline flag)
                if (mip.BulkData?.Data != null && mip.BulkData.Data.Length > 0)
                {
                    bool shouldWriteHere = mip.BulkData.Header?.DataResourceIndex >= 0 || 
                                           mip.BulkData.Header?.IsInline == true;
                    if (shouldWriteHere)
                    {
                        mip.BulkData.WriteData(writer);
                    }
                }
            }

            // For UE5.3+ DataResource format: write mip dimensions AFTER pixel data
            if (hasDataResources)
            {
                foreach (var mip in Mips)
                {
                    writer.Write(mip.SizeX);
                    writer.Write(mip.SizeY);
                    writer.Write(mip.SizeZ);
                }
                // bIsVirtual comes after dimensions
                writer.Write(bIsVirtual ? 1 : 0);
            }
        }

        public bool HasCpuCopy() => (PackedData & BitMask_HasCpuCopy) == BitMask_HasCpuCopy;
        public bool HasOptData() => (PackedData & BitMask_HasOptData) == BitMask_HasOptData;
        public bool IsCubemap() => (PackedData & BitMask_CubeMap) == BitMask_CubeMap;
        public int GetNumSlices() => (int)(PackedData & BitMask_NumSlices);

        /// <summary>
        /// Strip all mipmaps except the first one and convert to inline storage.
        /// This is used for texture mods to eliminate .ubulk dependencies.
        /// </summary>
        /// <returns>True if mipmaps were stripped, false if already had 1 or 0 mipmaps.</returns>
        public bool StripMipmaps()
        {
            if (Mips.Count <= 1)
            {
                return false;
            }

            // Keep only the first (largest) mipmap
            var firstMip = Mips[0];
            
            // Convert to inline storage
            firstMip.ConvertToInline();

            // Clear the mip list and add only the first mip
            Mips.Clear();
            Mips.Add(firstMip);

            // Update FirstMipToSerialize - should be 0 for single mip textures
            // This tells the engine that mip 0 is the first one to serialize (the only one we have)
            FirstMipToSerialize = 0;

            return true;
        }

        /// <summary>
        /// Get total size of all mipmap data.
        /// </summary>
        public long GetTotalMipDataSize()
        {
            long total = 0;
            foreach (var mip in Mips)
            {
                total += mip.BulkData?.Data?.Length ?? 0;
            }
            return total;
        }
    }

    /// <summary>
    /// Optional texture platform data for UE5+.
    /// </summary>
    public struct FOptTexturePlatformData
    {
        public uint ExtData;
        public uint NumMipsInTail;
    }
}
