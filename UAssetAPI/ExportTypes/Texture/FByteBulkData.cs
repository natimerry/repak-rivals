using System;
using System.IO;
using UAssetAPI.UnrealTypes;

namespace UAssetAPI.ExportTypes.Texture
{
    /// <summary>
    /// Bulk data container for texture mipmap data.
    /// Ported from CUE4Parse with write support added.
    /// </summary>
    public class FByteBulkData
    {
        public FByteBulkDataHeader Header;
        public byte[] Data;

        /// <summary>
        /// Path to external bulk file (.ubulk) if data is stored externally.
        /// </summary>
        public string ExternalBulkFilePath;

        public FByteBulkData()
        {
            Header = new FByteBulkDataHeader();
            Data = Array.Empty<byte>();
        }

        public FByteBulkData(byte[] data)
        {
            Header = new FByteBulkDataHeader();
            Data = data ?? Array.Empty<byte>();
            Header.ElementCount = Data.Length;
            Header.SizeOnDisk = Data.Length;
            Header.BulkDataFlags = EBulkDataFlags.BULKDATA_ForceInlinePayload;
        }

        public FByteBulkData(AssetBinaryReader reader, string bulkFilePath = null)
        {
            ExternalBulkFilePath = bulkFilePath;
            Read(reader);
        }

        public void Read(AssetBinaryReader reader)
        {
            Header = new FByteBulkDataHeader(reader);

            if (Header.ElementCount == 0 || Header.BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_Unused))
            {
                Data = Array.Empty<byte>();
                return;
            }

            // UE5.3+ DataResources: The header only wrote a 4-byte DataResourceIndex.
            // The pixel data is NOT at the current stream position — it's serialized after
            // all mip headers (for inline) or in .ubulk (for external). Don't read here;
            // FTexturePlatformData.Read() will read inline data from the correct position.
            if (Header.DataResourceIndex >= 0)
            {
                Data = Array.Empty<byte>();
                return;
            }

            // Legacy format: Check if data is inline by ForceInlinePayload flag
            bool isInline = Header.BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_ForceInlinePayload);

            if (isInline)
            {
                // Data is inline - read it directly from current position
                if (Header.ElementCount > 0 && Header.ElementCount < int.MaxValue)
                {
                    Data = reader.ReadBytes((int)Header.ElementCount);
                }
                else
                {
                    Data = Array.Empty<byte>();
                }
            }
            else if (!string.IsNullOrEmpty(ExternalBulkFilePath) && File.Exists(ExternalBulkFilePath))
            {
                // Data is in .ubulk file - read from the offset
                try
                {
                    using (var bulkReader = new BinaryReader(File.OpenRead(ExternalBulkFilePath)))
                    {
                        bulkReader.BaseStream.Seek(Header.OffsetInFile, SeekOrigin.Begin);
                        Data = bulkReader.ReadBytes((int)Header.ElementCount);
                    }
                }
                catch
                {
                    // Failed to read from ubulk - store empty
                    Data = Array.Empty<byte>();
                }
            }
            else
            {
                // External data but no ubulk file - store empty
                Data = Array.Empty<byte>();
            }
        }

        public void Write(AssetBinaryWriter writer)
        {
            // Update header with current data size
            Header.ElementCount = Data?.Length ?? 0;
            Header.SizeOnDisk = Header.ElementCount;

            Header.Write(writer);

            // For UE5.3+ with DataResourceIndex, the pixel data is written separately
            // at the end of the mip array, not inline with each mip's header.
            // The DataResource's SerialOffset points to where the data is.
            // Only write inline data here for legacy format (no DataResourceIndex)
            if (Header.DataResourceIndex < 0 && Header.IsInline && Data != null && Data.Length > 0)
            {
                writer.Write(Data);
            }
        }

        /// <summary>
        /// Write just the pixel data (for UE5.3+ format where data comes after all mip headers)
        /// </summary>
        public void WriteData(AssetBinaryWriter writer)
        {
            if (Data != null && Data.Length > 0)
            {
                writer.Write(Data);
            }
        }

        /// <summary>
        /// Convert bulk data to inline format (for mipmap stripping).
        /// This removes external file references and embeds data directly.
        /// </summary>
        public void ConvertToInline()
        {
            // Clear external file flags
            Header.BulkDataFlags &= ~EBulkDataFlags.BULKDATA_PayloadInSeperateFile;
            Header.BulkDataFlags &= ~EBulkDataFlags.BULKDATA_PayloadAtEndOfFile;
            Header.BulkDataFlags &= ~EBulkDataFlags.BULKDATA_OptionalPayload;
            
            // Set inline flag
            Header.BulkDataFlags |= EBulkDataFlags.BULKDATA_ForceInlinePayload;
            
            // Clear offset since data is now inline
            Header.OffsetInFile = 0;
            Header.CookedIndex = -1;
            
            // Keep DataResourceIndex for UE5.3+ - the DataResource will be updated with correct offset
            // Don't clear it here as it's needed for the Write to work correctly
        }

        /// <summary>
        /// Check if this bulk data has actual pixel data.
        /// </summary>
        public bool HasData => Data != null && Data.Length > 0;
    }
}
