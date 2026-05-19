using System;
using UAssetAPI.UnrealTypes;

namespace UAssetAPI.ExportTypes.Texture
{
    /// <summary>
    /// A single mipmap level for a texture.
    /// Ported from CUE4Parse with write support added.
    /// </summary>
    public class FTexture2DMipMap
    {
        public FByteBulkData BulkData;
        public int SizeX;
        public int SizeY;
        public int SizeZ;

        public FTexture2DMipMap()
        {
            BulkData = new FByteBulkData();
            SizeX = 0;
            SizeY = 0;
            SizeZ = 1;
        }

        public FTexture2DMipMap(FByteBulkData bulkData, int sizeX, int sizeY, int sizeZ = 1)
        {
            BulkData = bulkData;
            SizeX = sizeX;
            SizeY = sizeY;
            SizeZ = sizeZ;
        }

        public FTexture2DMipMap(AssetBinaryReader reader, string bulkFilePath = null, bool serializeMipData = true)
        {
            Read(reader, bulkFilePath, serializeMipData);
        }

        public void Read(AssetBinaryReader reader, string bulkFilePath = null, bool serializeMipData = true)
        {
            // Cooked flag (UE4 only, UE5 uses IsFilterEditorOnly)
            bool cooked = true;
            if (reader.Asset.ObjectVersionUE5 < ObjectVersionUE5.INITIAL_VERSION)
            {
                // UE4: read cooked bool
                if (reader.Asset.GetCustomVersion<FRenderingObjectVersion>() >= FRenderingObjectVersion.TextureSourceArtRefactor)
                {
                    cooked = reader.ReadInt32() != 0;
                }
            }

            // Read bulk data
            if (serializeMipData)
            {
                BulkData = new FByteBulkData(reader, bulkFilePath);
            }
            else
            {
                BulkData = new FByteBulkData();
            }

            // For UE5.3+ DataResources format, dimensions are serialized AFTER all mip
            // pixel data (not after each mip header). Skip here; FTexturePlatformData reads them.
            if (BulkData?.Header?.DataResourceIndex >= 0)
            {
                // Dimensions will be populated by FTexturePlatformData.Read() from the tail section
                return;
            }

            // Read dimensions
            SizeX = reader.ReadInt32();
            SizeY = reader.ReadInt32();
            
            // SizeZ added in UE4.20
            if (reader.Asset.ObjectVersionUE5 >= ObjectVersionUE5.INITIAL_VERSION ||
                reader.Asset.GetCustomVersion<FRenderingObjectVersion>() >= FRenderingObjectVersion.TextureSourceArtRefactor)
            {
                SizeZ = reader.ReadInt32();
            }
            else
            {
                SizeZ = 1;
            }

            // Derived data key (editor only, not cooked)
            if (!cooked && reader.Asset.GetCustomVersion<FRenderingObjectVersion>() >= FRenderingObjectVersion.TextureDerivedData2)
            {
                var derivedDataKey = reader.ReadFString();
            }
        }

        public void Write(AssetBinaryWriter writer)
        {
            // Write bulk data header (for UE5.3+ with DataResources, this writes only the data_resource_id)
            BulkData.Write(writer);

            // For UE5.3+ with DataResources, dimensions are NOT written after the bulk data header
            // The pixel data comes after all mip headers, pointed to by the DataResource's SerialOffset
            // Only write dimensions for legacy format (no DataResourceIndex)
            if (BulkData?.Header?.DataResourceIndex < 0)
            {
                // Write dimensions
                writer.Write(SizeX);
                writer.Write(SizeY);
                
                // SizeZ for UE4.20+
                if (writer.Asset.ObjectVersionUE5 >= ObjectVersionUE5.INITIAL_VERSION ||
                    writer.Asset.GetCustomVersion<FRenderingObjectVersion>() >= FRenderingObjectVersion.TextureSourceArtRefactor)
                {
                    writer.Write(SizeZ);
                }
            }
        }

        /// <summary>
        /// Get the pixel data for this mipmap.
        /// </summary>
        public byte[] GetData()
        {
            return BulkData?.Data ?? Array.Empty<byte>();
        }

        /// <summary>
        /// Set the pixel data for this mipmap.
        /// </summary>
        public void SetData(byte[] data)
        {
            if (BulkData == null)
            {
                BulkData = new FByteBulkData(data);
            }
            else
            {
                BulkData.Data = data;
                BulkData.Header.ElementCount = data?.Length ?? 0;
                BulkData.Header.SizeOnDisk = BulkData.Header.ElementCount;
            }
        }

        /// <summary>
        /// Convert this mipmap to inline storage.
        /// </summary>
        public void ConvertToInline()
        {
            BulkData?.ConvertToInline();
        }
    }

    /// <summary>
    /// Custom version for rendering objects.
    /// </summary>
    public enum FRenderingObjectVersion
    {
        BeforeCustomVersionWasAdded = 0,
        TextureSourceArtRefactor = 1,
        TextureDerivedData2 = 2,
    }
}
