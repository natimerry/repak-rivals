using System;
using UAssetAPI.UnrealTypes;

namespace UAssetAPI.ExportTypes.Texture
{
    /// <summary>
    /// Header for bulk data, containing flags, element count, size, and offset information.
    /// Ported from CUE4Parse.
    /// </summary>
    public class FByteBulkDataHeader
    {
        public EBulkDataFlags BulkDataFlags;
        public long ElementCount;
        public long SizeOnDisk;
        public long OffsetInFile;
        public int CookedIndex; // UE5.3+

        public FByteBulkDataHeader()
        {
            BulkDataFlags = EBulkDataFlags.BULKDATA_None;
            ElementCount = 0;
            SizeOnDisk = 0;
            OffsetInFile = 0;
            CookedIndex = -1;
        }

        public FByteBulkDataHeader(AssetBinaryReader reader)
        {
            Read(reader);
        }

        public void Read(AssetBinaryReader reader)
        {
            // UE5.3+ (Marvel Rivals): Use DataResources map if available
            // CUE4Parse reads bulk data metadata from DataResourceMap using an index
            var dataResources = reader.Asset?.DataResources;
            if (dataResources != null && dataResources.Count > 0)
            {
                // Read the data resource index
                int dataIndex = reader.ReadInt32();
                
                // IMPORTANT: Always store the original index for writing back
                // Even if the index is out of bounds, we need to preserve it
                if (dataIndex >= 0)
                {
                    DataResourceIndex = dataIndex;
                    
                    // Only look up metadata if index is valid
                    if (dataIndex < dataResources.Count)
                    {
                        var metaData = dataResources[dataIndex];
                        BulkDataFlags = (EBulkDataFlags)metaData.LegacyBulkDataFlags;
                        ElementCount = metaData.RawSize;
                        SizeOnDisk = metaData.SerialSize;
                        OffsetInFile = metaData.SerialOffset;
                        CookedIndex = metaData.CookedIndex;
                    }
                    return;
                }
                // Invalid index - rewind and try legacy parsing
                reader.BaseStream.Position -= 4;
            }

            // Legacy parsing for older UE versions
            BulkDataFlags = (EBulkDataFlags)reader.ReadUInt32();

            // Element count (number of bytes)
            if (BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_Size64Bit))
            {
                ElementCount = reader.ReadInt64();
            }
            else
            {
                ElementCount = reader.ReadInt32();
            }

            // Size on disk (may be compressed)
            if (BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_Size64Bit))
            {
                SizeOnDisk = reader.ReadInt64();
            }
            else
            {
                SizeOnDisk = reader.ReadUInt32();
            }

            // Offset in file
            OffsetInFile = reader.ReadInt64();

            // Apply bulk data start offset fix-up if needed (UE4.26+)
            if (!BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_NoOffsetFixUp))
            {
                OffsetInFile += reader.Asset?.BulkDataStartOffset ?? 0;
            }
        }

        /// <summary>
        /// Data resource index for UE5.3+ (written instead of legacy header)
        /// </summary>
        public int DataResourceIndex { get; set; } = -1;

        public void Write(AssetBinaryWriter writer)
        {
            // For UE5.3+ with DataResources, write just the data_resource_id (Int32)
            // The actual bulk data metadata is stored in the .uasset DataResources section
            if (DataResourceIndex >= 0)
            {
                writer.Write(DataResourceIndex);
                return;
            }

            // Legacy format for older UE versions or when DataResources not used
            writer.Write((uint)BulkDataFlags);
            writer.Write((int)ElementCount);
            writer.Write((int)SizeOnDisk);
            writer.Write(OffsetInFile);
        }

        /// <summary>
        /// Check if data is stored inline in the .uexp file.
        /// </summary>
        public bool IsInline => BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_ForceInlinePayload) ||
                                (!BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_PayloadInSeperateFile) &&
                                 !BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_PayloadAtEndOfFile) &&
                                 !BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_OptionalPayload));

        /// <summary>
        /// Check if data is stored in a separate .ubulk file.
        /// </summary>
        public bool IsInSeparateFile => BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_PayloadInSeperateFile);

        /// <summary>
        /// Check if data is stored in an optional .uptnl file.
        /// </summary>
        public bool IsOptional => BulkDataFlags.HasFlag(EBulkDataFlags.BULKDATA_OptionalPayload);
    }
}
