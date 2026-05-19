using Newtonsoft.Json;
using System;
using System.Collections;

namespace UAssetAPI.JSON
{
    public class BitArrayJsonConverter : JsonConverter<BitArray>
    {
        public override BitArray ReadJson(JsonReader reader, Type objectType, BitArray existingValue, bool hasExistingValue, JsonSerializer serializer)
        {
            var bools = serializer.Deserialize<bool[]>(reader);
            return bools != null ? new BitArray(bools) : new BitArray(0);
        }

        public override void WriteJson(JsonWriter writer, BitArray value, JsonSerializer serializer)
        {
            bool[] bools = new bool[value.Length];
            value.CopyTo(bools, 0);
            serializer.Serialize(writer, bools);
        }
    }
}
