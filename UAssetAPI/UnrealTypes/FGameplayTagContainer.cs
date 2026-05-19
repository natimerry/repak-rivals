using System;
using System.Collections.Generic;

namespace UAssetAPI.UnrealTypes
{
    /// <summary>
    /// FGameplayTag - A single gameplay tag, which is just an FName wrapper.
    /// </summary>
    public class FGameplayTag
    {
        public FName TagName;

        public FGameplayTag()
        {
            TagName = null;
        }

        public FGameplayTag(FName tagName)
        {
            TagName = tagName;
        }

        public FGameplayTag(AssetBinaryReader reader)
        {
            TagName = reader.ReadFName();
        }

        public void Write(AssetBinaryWriter writer)
        {
            writer.Write(TagName);
        }

        public bool IsValid()
        {
            return TagName != null && TagName.Value != null;
        }

        public override string ToString()
        {
            return TagName?.ToString() ?? "None";
        }

        /// <summary>
        /// Serialized size: FName = 8 bytes (4 byte index + 4 byte number)
        /// </summary>
        public static int SerializedSize => 8;
    }

    /// <summary>
    /// FGameplayTagContainer - A container holding multiple gameplay tags.
    /// 
    /// Binary format (per CUE4Parse):
    /// - int32 count
    /// - FGameplayTag[count] (each tag is an FName = 8 bytes)
    /// 
    /// Empty container = 4 bytes (just count = 0)
    /// 
    /// NOTE: ParentTags is a runtime-only cached field and is NOT serialized.
    /// </summary>
    public class FGameplayTagContainer
    {
        /// <summary>
        /// The gameplay tags stored in this container.
        /// </summary>
        public List<FGameplayTag> GameplayTags;

        public FGameplayTagContainer()
        {
            GameplayTags = new List<FGameplayTag>();
        }

        public FGameplayTagContainer(AssetBinaryReader reader)
        {
            int count = reader.ReadInt32();
            GameplayTags = new List<FGameplayTag>(count);
            for (int i = 0; i < count; i++)
            {
                GameplayTags.Add(new FGameplayTag(reader));
            }
        }

        public void Write(AssetBinaryWriter writer)
        {
            writer.Write(GameplayTags?.Count ?? 0);
            if (GameplayTags != null)
            {
                foreach (var tag in GameplayTags)
                {
                    tag.Write(writer);
                }
            }
        }

        /// <summary>
        /// Get the serialized size of this container.
        /// </summary>
        public int GetSerializedSize()
        {
            // 4 bytes for count + 8 bytes per tag
            return 4 + ((GameplayTags?.Count ?? 0) * FGameplayTag.SerializedSize);
        }

        /// <summary>
        /// Size of an empty container: just int32 count = 0
        /// </summary>
        public static int EmptySerializedSize => 4;

        public bool IsEmpty()
        {
            return GameplayTags == null || GameplayTags.Count == 0;
        }

        public bool HasTag(FGameplayTag tag)
        {
            if (tag == null || !tag.IsValid() || GameplayTags == null)
                return false;

            foreach (var t in GameplayTags)
            {
                if (t.TagName?.ToString() == tag.TagName?.ToString())
                    return true;
            }
            return false;
        }

        public override string ToString()
        {
            if (GameplayTags == null || GameplayTags.Count == 0)
                return "Empty";
            return string.Join(", ", GameplayTags);
        }
    }
}
