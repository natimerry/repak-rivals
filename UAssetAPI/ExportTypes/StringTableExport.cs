using Newtonsoft.Json;
using System.Collections.Generic;
using System.Linq;
using UAssetAPI.UnrealTypes;
using UAssetAPI.ExportTypes;

namespace UAssetAPI.ExportTypes
{
    /// <summary>
    /// A string table. Holds Key->SourceString pairs of text.
    /// Extends TMap&lt;FString, FString&gt; to maintain IOrderedDictionary interface
    /// compatible with upstream UAssetAPI and consumer tools (UAssetGUI, etc.).
    /// </summary>
    public class FStringTable : TMap<FString, FString>
    {
        [JsonProperty]
        public FString TableNamespace;

        /// <summary>
        /// Per-entry FGameplayTagContainer data (Marvel Rivals extension).
        /// Keyed by the string table key. Null if the source asset
        /// did not contain gameplay tag containers (standard UE5 format).
        /// </summary>
        [JsonProperty]
        public List<FGameplayTagContainer> EntryGameplayTags;

        /// <summary>
        /// Trailing FGameplayTagContainer after all entries (Marvel Rivals extension).
        /// Null if the source asset did not contain gameplay tag containers.
        /// </summary>
        [JsonProperty]
        public FGameplayTagContainer TrailingTagContainer;

        /// <summary>
        /// Whether this string table was read with FGameplayTagContainer data.
        /// </summary>
        [JsonProperty]
        public bool HasGameplayTags;

        public FStringTable(FString tableNamespace) : base()
        {
            TableNamespace = tableNamespace;
        }

        public FStringTable() : base()
        {
        }
    }

    /// <summary>
    /// Export data for a string table. See <see cref="FStringTable"/>.
    /// </summary>
    public class StringTableExport : NormalExport
    {
        [JsonProperty]
        public FStringTable Table;

        public StringTableExport(Export super) : base(super)
        {

        }

        public StringTableExport(FStringTable data, UAsset asset, byte[] extras) : base(asset, extras)
        {
            Table = data;
        }

        public StringTableExport()
        {

        }

        public override void Read(AssetBinaryReader reader, int nextStarting)
        {
            base.Read(reader, nextStarting);

            Table = new FStringTable(reader.ReadFString());

            int numEntries = reader.ReadInt32();
            long posBeforeEntries = reader.BaseStream.Position;

            // Try reading with interleaved FGameplayTagContainers (Marvel Rivals format):
            // [Key, Value, Tags] per entry, then trailing Tags
            bool hasGameplayTags = false;
            var keys = new List<FString>(numEntries);
            var values = new List<FString>(numEntries);

            try
            {
                var trialKeys = new List<FString>(numEntries);
                var trialValues = new List<FString>(numEntries);
                var trialTags = new List<FGameplayTagContainer>(numEntries);

                for (int i = 0; i < numEntries; i++)
                {
                    trialKeys.Add(reader.ReadFString());
                    trialValues.Add(reader.ReadFString());
                    trialTags.Add(new FGameplayTagContainer(reader));
                }
                var trialTrailing = new FGameplayTagContainer(reader);

                if (reader.BaseStream.Position == nextStarting)
                {
                    hasGameplayTags = true;
                    keys = trialKeys;
                    values = trialValues;
                    Table.EntryGameplayTags = trialTags;
                    Table.TrailingTagContainer = trialTrailing;
                }
            }
            catch
            {
                // Interleaved tag reading failed
            }

            // Fall back to standard UE5 format: [Key, Value] per entry, no tags
            if (!hasGameplayTags)
            {
                reader.BaseStream.Position = posBeforeEntries;
                keys.Clear();
                values.Clear();

                for (int i = 0; i < numEntries; i++)
                {
                    keys.Add(reader.ReadFString());
                    values.Add(reader.ReadFString());
                }
            }

            Table.HasGameplayTags = hasGameplayTags;

            // Add entries to the TMap (handles duplicate keys by overwriting)
            for (int i = 0; i < numEntries; i++)
            {
                var key = keys[i];
                if (key == null) continue;
                if (Table.ContainsKey(key))
                    Table[key] = values[i];
                else
                    Table.Add(key, values[i]);
            }
        }

        public override void Write(AssetBinaryWriter writer)
        {
            base.Write(writer);

            writer.Write(Table.TableNamespace);
            writer.Write(Table.Count);
            for (int i = 0; i < Table.Count; i++)
            {
                writer.Write(Table.Keys.ElementAt(i));
                writer.Write(Table[i]);

                if (Table.HasGameplayTags)
                {
                    var tags = (Table.EntryGameplayTags != null && i < Table.EntryGameplayTags.Count)
                        ? Table.EntryGameplayTags[i]
                        : new FGameplayTagContainer();
                    tags.Write(writer);
                }
            }

            if (Table.HasGameplayTags)
            {
                var trailing = Table.TrailingTagContainer ?? new FGameplayTagContainer();
                trailing.Write(writer);
            }
        }
    }
}
