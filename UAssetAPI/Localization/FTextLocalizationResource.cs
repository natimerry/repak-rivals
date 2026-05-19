using System;
using System.Collections.Generic;
using System.IO;
using System.Text;

namespace UAssetAPI.Localization
{
    /// <summary>
    /// Version enum for .locres files.
    /// </summary>
    public enum ELocResVersion : byte
    {
        /// <summary>Legacy format file - will be missing the magic number.</summary>
        Legacy = 0,
        /// <summary>Compact format file - strings are stored in a LUT to avoid duplication.</summary>
        Compact,
        /// <summary>Optimized format file - namespaces/keys are pre-hashed (CRC32).</summary>
        Optimized_CRC32,
        /// <summary>Optimized format file - namespaces/keys are pre-hashed (CityHash64, UTF-16).</summary>
        Optimized_CityHash64_UTF16,

        LatestPlusOne,
        Latest = LatestPlusOne - 1
    }

    /// <summary>
    /// An entry in a .locres string lookup table.
    /// </summary>
    public class LocResString
    {
        public string Value;
        public int RefCount;

        public LocResString(string value, int refCount)
        {
            Value = value;
            RefCount = refCount;
        }
    }

    /// <summary>
    /// A single localization entry with its source string hash and localized text.
    /// </summary>
    public class LocResEntry
    {
        public uint SourceStringHash;
        public string LocalizedString;

        public LocResEntry(uint sourceStringHash, string localizedString)
        {
            SourceStringHash = sourceStringHash;
            LocalizedString = localizedString;
        }
    }

    /// <summary>
    /// Parser for Unreal Engine .locres (FTextLocalizationResource) files.
    /// Structure: Dictionary&lt;Namespace, Dictionary&lt;Key, LocResEntry&gt;&gt;
    /// </summary>
    public class FTextLocalizationResource
    {
        private static readonly byte[] LocResMagicBytes = new byte[]
        {
            0x0E, 0x14, 0x74, 0x75,  // 0x7574140E
            0x67, 0x4A, 0x03, 0xFC,  // 0xFC034A67
            0x4A, 0x15, 0x90, 0x9D,  // 0x9D90154A
            0xC3, 0x37, 0x7F, 0x1B   // 0x1B7F37C3
        };

        /// <summary>
        /// The version of this .locres file.
        /// </summary>
        public ELocResVersion Version { get; private set; }

        /// <summary>
        /// All entries: Namespace → (Key → LocResEntry).
        /// </summary>
        public Dictionary<string, Dictionary<string, LocResEntry>> Entries { get; private set; }

        /// <summary>
        /// Total number of localized strings across all namespaces.
        /// </summary>
        public int TotalEntryCount
        {
            get
            {
                int count = 0;
                foreach (var ns in Entries.Values)
                    count += ns.Count;
                return count;
            }
        }

        /// <summary>
        /// Load a .locres file from the given path.
        /// </summary>
        public FTextLocalizationResource(string filePath)
        {
            using var stream = File.OpenRead(filePath);
            using var reader = new BinaryReader(stream, Encoding.UTF8, leaveOpen: false);
            Parse(reader);
        }

        /// <summary>
        /// Load a .locres file from a byte array.
        /// </summary>
        public FTextLocalizationResource(byte[] data)
        {
            using var stream = new MemoryStream(data);
            using var reader = new BinaryReader(stream, Encoding.UTF8, leaveOpen: false);
            Parse(reader);
        }

        /// <summary>
        /// Load a .locres file from a stream.
        /// </summary>
        public FTextLocalizationResource(Stream stream)
        {
            using var reader = new BinaryReader(stream, Encoding.UTF8, leaveOpen: true);
            Parse(reader);
        }

        /// <summary>
        /// Try to look up a localized string by namespace and key.
        /// </summary>
        public bool TryGetString(string namespaceName, string key, out string localizedString)
        {
            localizedString = null;
            if (Entries.TryGetValue(namespaceName, out var keys))
            {
                if (keys.TryGetValue(key, out var entry))
                {
                    localizedString = entry.LocalizedString;
                    return true;
                }
            }
            return false;
        }

        /// <summary>
        /// Get all entries as a flat list of (namespace, key, localizedString) tuples.
        /// </summary>
        public IEnumerable<(string Namespace, string Key, string LocalizedString)> GetAllEntries()
        {
            foreach (var nsPair in Entries)
            {
                foreach (var keyPair in nsPair.Value)
                {
                    yield return (nsPair.Key, keyPair.Key, keyPair.Value.LocalizedString);
                }
            }
        }

        private void Parse(BinaryReader reader)
        {
            Entries = new Dictionary<string, Dictionary<string, LocResEntry>>();

            // Check magic
            var version = ELocResVersion.Legacy;
            byte[] magicBytes = reader.ReadBytes(16);
            if (MagicMatches(magicBytes))
            {
                version = (ELocResVersion)reader.ReadByte();
            }
            else
            {
                // Legacy format - seek back
                reader.BaseStream.Position = 0;
            }
            Version = version;

            if (version > ELocResVersion.Latest)
                throw new InvalidDataException($"LocRes file is too new (version {(int)version}, max supported {(int)ELocResVersion.Latest})");

            // Read localized string array (LUT)
            LocResString[] stringArray = Array.Empty<LocResString>();
            if (version >= ELocResVersion.Compact)
            {
                stringArray = ReadStringArray(reader, version);
            }

            // Skip total entries count
            if (version >= ELocResVersion.Optimized_CRC32)
            {
                reader.ReadInt32(); // EntriesCount (unused, we count per-namespace)
            }

            // Read namespaces
            uint namespaceCount = reader.ReadUInt32();
            for (uint i = 0; i < namespaceCount; i++)
            {
                string namespaceName = ReadTextKey(reader, version);
                uint keyCount = reader.ReadUInt32();

                var keyEntries = new Dictionary<string, LocResEntry>((int)keyCount);

                for (uint j = 0; j < keyCount; j++)
                {
                    string key = ReadTextKey(reader, version);
                    uint sourceStringHash = reader.ReadUInt32();
                    string localizedString;

                    if (version >= ELocResVersion.Compact)
                    {
                        int stringIndex = reader.ReadInt32();
                        if (stringIndex >= 0 && stringIndex < stringArray.Length)
                        {
                            localizedString = stringArray[stringIndex].Value;
                            if (stringArray[stringIndex].RefCount != -1)
                                stringArray[stringIndex].RefCount--;
                        }
                        else
                        {
                            localizedString = string.Empty;
                        }
                    }
                    else
                    {
                        localizedString = ReadFString(reader);
                    }

                    keyEntries[key] = new LocResEntry(sourceStringHash, localizedString);
                }

                Entries[namespaceName] = keyEntries;
            }
        }

        private static LocResString[] ReadStringArray(BinaryReader reader, ELocResVersion version)
        {
            long offset = reader.ReadInt64();
            if (offset == -1)
                return Array.Empty<LocResString>();

            long savedPos = reader.BaseStream.Position;
            reader.BaseStream.Position = offset;

            int count = reader.ReadInt32();
            var arr = new LocResString[count];
            for (int i = 0; i < count; i++)
            {
                string str = ReadFString(reader);
                int refCount = version >= ELocResVersion.Optimized_CRC32 ? reader.ReadInt32() : -1;
                arr[i] = new LocResString(str, refCount);
            }

            reader.BaseStream.Position = savedPos;
            return arr;
        }

        private static string ReadTextKey(BinaryReader reader, ELocResVersion version)
        {
            if (version >= ELocResVersion.Optimized_CRC32)
            {
                reader.ReadUInt32(); // hash, skip
            }
            return ReadFString(reader);
        }

        /// <summary>
        /// Read an FString from the binary stream (UE format: int32 length, then chars).
        /// Negative length indicates UTF-16 encoding.
        /// </summary>
        private static string ReadFString(BinaryReader reader)
        {
            int length = reader.ReadInt32();
            if (length == 0) return string.Empty;

            if (length < 0)
            {
                // UTF-16
                int charCount = -length;
                byte[] data = reader.ReadBytes(charCount * 2);
                // Strip null terminator
                return Encoding.Unicode.GetString(data, 0, (charCount - 1) * 2);
            }
            else
            {
                // UTF-8 / Latin1
                byte[] data = reader.ReadBytes(length);
                // Strip null terminator
                return Encoding.UTF8.GetString(data, 0, length - 1);
            }
        }

        private static bool MagicMatches(byte[] bytes)
        {
            if (bytes.Length < 16) return false;
            for (int i = 0; i < 16; i++)
            {
                if (bytes[i] != LocResMagicBytes[i]) return false;
            }
            return true;
        }

        /// <summary>
        /// Write all entries to a JSON file for inspection.
        /// </summary>
        public string ToJson(bool indented = true)
        {
            var sb = new StringBuilder();
            sb.AppendLine("{");

            bool firstNs = true;
            foreach (var nsPair in Entries)
            {
                if (!firstNs) sb.AppendLine(",");
                firstNs = false;

                sb.Append($"  {EscapeJson(nsPair.Key)}: {{");
                sb.AppendLine();

                bool firstKey = true;
                foreach (var keyPair in nsPair.Value)
                {
                    if (!firstKey) sb.AppendLine(",");
                    firstKey = false;

                    sb.Append($"    {EscapeJson(keyPair.Key)}: {EscapeJson(keyPair.Value.LocalizedString)}");
                }
                sb.AppendLine();
                sb.Append("  }");
            }

            sb.AppendLine();
            sb.AppendLine("}");
            return sb.ToString();
        }

        private static string EscapeJson(string s)
        {
            if (s == null) return "null";
            return "\"" + s.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\n", "\\n").Replace("\r", "\\r").Replace("\t", "\\t") + "\"";
        }
    }
}
