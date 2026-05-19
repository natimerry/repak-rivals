using System;

namespace UAssetAPI.ExportTypes.Texture
{
    /// <summary>
    /// Flags serialized with bulk data.
    /// Ported from CUE4Parse and UE4-DDS-Tools.
    /// </summary>
    [Flags]
    public enum EBulkDataFlags : uint
    {
        BULKDATA_None = 0,
        /// <summary>If set, payload is stored at the end of the file and not inline.</summary>
        BULKDATA_PayloadAtEndOfFile = 1 << 0,
        /// <summary>Bulk data is compressed using ZLIB.</summary>
        BULKDATA_SerializeCompressedZLIB = 1 << 1,
        /// <summary>Force single element serialization.</summary>
        BULKDATA_ForceSingleElementSerialization = 1 << 2,
        /// <summary>Bulk data is only used once at runtime.</summary>
        BULKDATA_SingleUse = 1 << 3,
        /// <summary>Bulk data is compressed using LZO (deprecated).</summary>
        BULKDATA_CompressedLZO = 1 << 4,
        /// <summary>Bulk data will not be loaded at all.</summary>
        BULKDATA_Unused = 1 << 5,
        /// <summary>Bulk data is stored inline and should be serialized with the rest of the export.</summary>
        BULKDATA_ForceInlinePayload = 1 << 6,
        /// <summary>Force stream payload (opposite of inline).</summary>
        BULKDATA_ForceStreamPayload = 1 << 7,
        /// <summary>Bulk data is stored in a separate file (.ubulk).</summary>
        BULKDATA_PayloadInSeperateFile = 1 << 8,
        /// <summary>Bulk data is compressed with bit window.</summary>
        BULKDATA_SerializeCompressedBitWindow = 1 << 9,
        /// <summary>Force NOT inline payload.</summary>
        BULKDATA_Force_NOT_InlinePayload = 1 << 10,
        /// <summary>Bulk data is stored in a separate file (.uptnl).</summary>
        BULKDATA_OptionalPayload = 1 << 11,
        /// <summary>Bulk data is stored in memory-mapped file.</summary>
        BULKDATA_MemoryMappedPayload = 1 << 12,
        /// <summary>Size is 64-bit.</summary>
        BULKDATA_Size64Bit = 1 << 13,
        /// <summary>Duplicate non-optional payload that was stored in optional storage.</summary>
        BULKDATA_DuplicateNonOptionalPayload = 1 << 14,
        /// <summary>Indicates that an old ID is present.</summary>
        BULKDATA_BadDataVersion = 1 << 15,
        /// <summary>Indicates that the bulk data does not have a FIoChunkId.</summary>
        BULKDATA_NoOffsetFixUp = 1 << 16,
        /// <summary>Workspace domain payload.</summary>
        BULKDATA_WorkspaceDomainPayload = 1 << 17,
        /// <summary>Bulk data can be loaded lazily.</summary>
        BULKDATA_LazyLoadable = 1 << 18,
        /// <summary>Always allow discard.</summary>
        BULKDATA_AlwaysAllowDiscard = 1 << 28,
        /// <summary>Has async read pending.</summary>
        BULKDATA_HasAsyncReadPending = 1 << 29,
        /// <summary>Indicates that the bulk data is stored in a data resource.</summary>
        BULKDATA_DataIsMemoryMapped = 1 << 30,
    }
}
