using System;
using System.Collections.Generic;
using System.IO;
using UAssetAPI.UnrealTypes;

namespace UAssetAPI.ExportTypes
{
    /// <summary>
    /// Strip data flags used to determine what data was stripped during cooking.
    /// </summary>
    public class FStripDataFlags
    {
        public byte GlobalStripFlags;
        public byte ClassStripFlags;

        public FStripDataFlags()
        {
            GlobalStripFlags = 0;
            ClassStripFlags = 0;
        }

        public FStripDataFlags(AssetBinaryReader reader)
        {
            GlobalStripFlags = reader.ReadByte();
            ClassStripFlags = reader.ReadByte();
        }

        public void Write(AssetBinaryWriter writer)
        {
            writer.Write(GlobalStripFlags);
            writer.Write(ClassStripFlags);
        }

        public bool IsEditorDataStripped() => (GlobalStripFlags & 1) != 0;
        public bool IsDataStrippedForServer() => (GlobalStripFlags & 2) != 0;
        public bool IsClassDataStripped(byte flag) => (ClassStripFlags & flag) != 0;

        public static int SerializedSize => 2;
    }

    /// <summary>
    /// A bounding box and bounding sphere with the same origin.
    /// </summary>
    public class FBoxSphereBounds
    {
        /// <summary>
        /// The center of the bounding box/sphere.
        /// </summary>
        public FVector Origin;

        /// <summary>
        /// Half the size of the bounding box.
        /// </summary>
        public FVector BoxExtent;

        /// <summary>
        /// The radius of the bounding sphere.
        /// </summary>
        public double SphereRadius;

        public FBoxSphereBounds()
        {
            Origin = new FVector(0, 0, 0);
            BoxExtent = new FVector(0, 0, 0);
            SphereRadius = 0;
        }

        public FBoxSphereBounds(AssetBinaryReader reader)
        {
            Origin = new FVector(reader);
            BoxExtent = new FVector(reader);
            if (reader.Asset.ObjectVersionUE5 >= ObjectVersionUE5.LARGE_WORLD_COORDINATES)
            {
                SphereRadius = reader.ReadDouble();
            }
            else
            {
                SphereRadius = reader.ReadSingle();
            }
        }

        public int Write(AssetBinaryWriter writer)
        {
            int size = 0;
            size += Origin.Write(writer);
            size += BoxExtent.Write(writer);
            if (writer.Asset.ObjectVersionUE5 >= ObjectVersionUE5.LARGE_WORLD_COORDINATES)
            {
                writer.Write(SphereRadius);
                size += sizeof(double);
            }
            else
            {
                writer.Write((float)SphereRadius);
                size += sizeof(float);
            }
            return size;
        }

        /// <summary>
        /// Size depends on LWC: 3*4 + 3*4 + 4 = 28 bytes (float) or 3*8 + 3*8 + 8 = 56 bytes (double)
        /// </summary>
        public static int GetSerializedSize(bool useLargeWorldCoordinates)
        {
            return useLargeWorldCoordinates ? 56 : 28;
        }
    }

    /// <summary>
    /// Information about a single bone in the skeleton.
    /// </summary>
    public class FMeshBoneInfo
    {
        /// <summary>
        /// Name of the bone.
        /// </summary>
        public FName Name;

        /// <summary>
        /// Index of the parent bone (INDEX_NONE = -1 for root).
        /// </summary>
        public int ParentIndex;

        public FMeshBoneInfo()
        {
            Name = null;
            ParentIndex = -1;
        }

        public FMeshBoneInfo(AssetBinaryReader reader)
        {
            Name = reader.ReadFName();
            ParentIndex = reader.ReadInt32();
        }

        public void Write(AssetBinaryWriter writer)
        {
            writer.Write(Name);
            writer.Write(ParentIndex);
        }

        /// <summary>
        /// Size: FName (8 bytes) + int32 (4 bytes) = 12 bytes
        /// </summary>
        public static int SerializedSize => 12;
    }

    /// <summary>
    /// Reference skeleton containing bone hierarchy and reference pose.
    /// This is the core skeletal data that defines the bone structure.
    /// </summary>
    public class FReferenceSkeleton
    {
        /// <summary>
        /// Array of bone info (name and parent index).
        /// </summary>
        public List<FMeshBoneInfo> RefBoneInfo;

        /// <summary>
        /// Array of reference pose transforms (one per bone).
        /// </summary>
        public List<FTransform> RefBonePose;

        /// <summary>
        /// Map from bone name to bone index for fast lookup.
        /// </summary>
        public Dictionary<FName, int> NameToIndexMap;

        public FReferenceSkeleton()
        {
            RefBoneInfo = new List<FMeshBoneInfo>();
            RefBonePose = new List<FTransform>();
            NameToIndexMap = new Dictionary<FName, int>();
        }

        public FReferenceSkeleton(AssetBinaryReader reader)
        {
            // Read RefBoneInfo array
            int boneInfoCount = reader.ReadInt32();
            RefBoneInfo = new List<FMeshBoneInfo>(boneInfoCount);
            for (int i = 0; i < boneInfoCount; i++)
            {
                RefBoneInfo.Add(new FMeshBoneInfo(reader));
            }

            // Read RefBonePose array
            int bonePoseCount = reader.ReadInt32();
            RefBonePose = new List<FTransform>(bonePoseCount);
            for (int i = 0; i < bonePoseCount; i++)
            {
                RefBonePose.Add(new FTransform(reader));
            }

            // Read NameToIndexMap (TMap<FName, int32>)
            int mapCount = reader.ReadInt32();
            NameToIndexMap = new Dictionary<FName, int>(mapCount);
            for (int i = 0; i < mapCount; i++)
            {
                FName name = reader.ReadFName();
                int index = reader.ReadInt32();
                if (name != null && !NameToIndexMap.ContainsKey(name))
                {
                    NameToIndexMap[name] = index;
                }
            }
        }

        public void Write(AssetBinaryWriter writer)
        {
            // Write RefBoneInfo array
            writer.Write(RefBoneInfo.Count);
            foreach (var boneInfo in RefBoneInfo)
            {
                boneInfo.Write(writer);
            }

            // Write RefBonePose array
            writer.Write(RefBonePose.Count);
            foreach (var bonePose in RefBonePose)
            {
                bonePose.Write(writer);
            }

            // Write NameToIndexMap
            writer.Write(NameToIndexMap.Count);
            foreach (var kvp in NameToIndexMap)
            {
                writer.Write(kvp.Key);
                writer.Write(kvp.Value);
            }
        }

        /// <summary>
        /// Get the number of bones in the skeleton.
        /// </summary>
        public int BoneCount => RefBoneInfo.Count;

        /// <summary>
        /// Find bone index by name.
        /// </summary>
        public int FindBoneIndex(FName boneName)
        {
            if (boneName != null && NameToIndexMap.TryGetValue(boneName, out int index))
            {
                return index;
            }
            return -1;
        }

        /// <summary>
        /// Get bone info by index.
        /// </summary>
        public FMeshBoneInfo GetBoneInfo(int index)
        {
            if (index >= 0 && index < RefBoneInfo.Count)
            {
                return RefBoneInfo[index];
            }
            return null;
        }

        /// <summary>
        /// Get bone transform by index.
        /// </summary>
        public FTransform? GetBonePose(int index)
        {
            if (index >= 0 && index < RefBonePose.Count)
            {
                return RefBonePose[index];
            }
            return null;
        }
    }

    /// <summary>
    /// LOD settings for skeletal mesh.
    /// </summary>
    public class FSkeletalMeshLODInfo
    {
        /// <summary>
        /// Reduction settings for this LOD.
        /// </summary>
        public float ScreenSize;

        /// <summary>
        /// LOD hysteresis value.
        /// </summary>
        public float LODHysteresis;

        /// <summary>
        /// Bones to remove for this LOD.
        /// </summary>
        public List<FName> BonesToRemove;

        /// <summary>
        /// Bones to prioritize for this LOD.
        /// </summary>
        public List<FName> BonesToPrioritize;

        /// <summary>
        /// Weight threshold for bone prioritization.
        /// </summary>
        public float WeightOfPrioritization;

        /// <summary>
        /// Mapping from section index to material index.
        /// </summary>
        public List<int> LODMaterialMap;

        public FSkeletalMeshLODInfo()
        {
            ScreenSize = 1.0f;
            LODHysteresis = 0.0f;
            BonesToRemove = new List<FName>();
            BonesToPrioritize = new List<FName>();
            WeightOfPrioritization = 1.0f;
            LODMaterialMap = new List<int>();
        }
    }

    /// <summary>
    /// Section info for a skeletal mesh LOD.
    /// </summary>
    public class FSkelMeshSection
    {
        /// <summary>
        /// Material index for this section.
        /// </summary>
        public short MaterialIndex;

        /// <summary>
        /// First index in the index buffer.
        /// </summary>
        public int BaseIndex;

        /// <summary>
        /// Number of triangles in this section.
        /// </summary>
        public int NumTriangles;

        /// <summary>
        /// Whether this section is disabled.
        /// </summary>
        public bool bDisabled;

        /// <summary>
        /// Whether this section casts shadow.
        /// </summary>
        public bool bCastShadow;

        /// <summary>
        /// Whether this section is visible in ray tracing.
        /// </summary>
        public bool bVisibleInRayTracing;

        /// <summary>
        /// Base vertex index for this section.
        /// </summary>
        public uint BaseVertexIndex;

        /// <summary>
        /// Cloth mapping data GUID.
        /// </summary>
        public Guid ClothMappingDataGuid;

        /// <summary>
        /// Number of vertices in this section.
        /// </summary>
        public int NumVertices;

        /// <summary>
        /// Maximum bone influences for this section.
        /// </summary>
        public int MaxBoneInfluences;

        /// <summary>
        /// Corresponding cloth asset index.
        /// </summary>
        public short CorrespondClothAssetIndex;

        /// <summary>
        /// Cloth asset submitted vertex indices.
        /// </summary>
        public List<short> ClothingData;

        public FSkelMeshSection()
        {
            MaterialIndex = 0;
            BaseIndex = 0;
            NumTriangles = 0;
            bDisabled = false;
            bCastShadow = true;
            bVisibleInRayTracing = true;
            BaseVertexIndex = 0;
            ClothMappingDataGuid = Guid.Empty;
            NumVertices = 0;
            MaxBoneInfluences = 4;
            CorrespondClothAssetIndex = -1;
            ClothingData = new List<short>();
        }
    }

    /// <summary>
    /// Vertex buffer flags for skeletal mesh.
    /// </summary>
    [Flags]
    public enum ESkeletalMeshVertexFlags : uint
    {
        None = 0,
        UseFullPrecisionUVs = 1 << 0,
        HasVertexColors = 1 << 1,
        UseHighPrecisionTangentBasis = 1 << 2,
        UseHighPrecisionWeights = 1 << 3,
    }

    /// <summary>
    /// Skeletal mesh vertex data.
    /// </summary>
    public class FSkeletalMeshVertexBuffer
    {
        /// <summary>
        /// Number of texture coordinates.
        /// </summary>
        public int NumTexCoords;

        /// <summary>
        /// Whether to use full precision UVs.
        /// </summary>
        public bool bUseFullPrecisionUVs;

        /// <summary>
        /// Whether the mesh has extra bone influences.
        /// </summary>
        public bool bExtraBoneInfluences;

        /// <summary>
        /// Number of vertices.
        /// </summary>
        public int NumVertices;

        /// <summary>
        /// Raw vertex data (positions, tangents, UVs, etc.).
        /// </summary>
        public byte[] VertexData;

        public FSkeletalMeshVertexBuffer()
        {
            NumTexCoords = 1;
            bUseFullPrecisionUVs = false;
            bExtraBoneInfluences = false;
            NumVertices = 0;
            VertexData = Array.Empty<byte>();
        }
    }

    /// <summary>
    /// Skin weight buffer for skeletal mesh.
    /// </summary>
    public class FSkinWeightVertexBuffer
    {
        /// <summary>
        /// Whether to use 16-bit bone indices.
        /// </summary>
        public bool bUse16BitBoneIndex;

        /// <summary>
        /// Number of bone influences per vertex.
        /// </summary>
        public int NumBoneInfluences;

        /// <summary>
        /// Number of vertices.
        /// </summary>
        public int NumVertices;

        /// <summary>
        /// Raw weight data.
        /// </summary>
        public byte[] WeightData;

        public FSkinWeightVertexBuffer()
        {
            bUse16BitBoneIndex = false;
            NumBoneInfluences = 4;
            NumVertices = 0;
            WeightData = Array.Empty<byte>();
        }
    }

    /// <summary>
    /// Color vertex buffer for skeletal mesh.
    /// </summary>
    public class FColorVertexBuffer
    {
        /// <summary>
        /// Stride between vertices in bytes.
        /// </summary>
        public int Stride;

        /// <summary>
        /// Number of vertices.
        /// </summary>
        public int NumVertices;

        /// <summary>
        /// Raw color data (BGRA format).
        /// </summary>
        public byte[] ColorData;

        public FColorVertexBuffer()
        {
            Stride = 4;
            NumVertices = 0;
            ColorData = Array.Empty<byte>();
        }
    }

    /// <summary>
    /// Active bone indices for a skeletal mesh LOD.
    /// </summary>
    public class FBoneReference
    {
        /// <summary>
        /// Name of the bone.
        /// </summary>
        public FName BoneName;

        public FBoneReference()
        {
            BoneName = null;
        }

        public FBoneReference(AssetBinaryReader reader)
        {
            BoneName = reader.ReadFName();
        }

        public void Write(AssetBinaryWriter writer)
        {
            writer.Write(BoneName);
        }
    }
}
