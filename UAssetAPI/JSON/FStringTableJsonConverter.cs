using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using System;
using System.Collections;
using System.Collections.Generic;
using System.Collections.Specialized;
using UAssetAPI.UnrealTypes;
using UAssetAPI.ExportTypes;

namespace UAssetAPI.JSON
{
    public class FStringTableJsonConverter : JsonConverter
    {
        public override bool CanConvert(Type objectType)
        {
            return objectType == typeof(FStringTable);
        }

        public override void WriteJson(JsonWriter writer, object value, JsonSerializer serializer)
        {
            var realVal = (FStringTable)value;

            ICollection keys = ((IOrderedDictionary)value).Keys;
            ICollection values = ((IOrderedDictionary)value).Values;
            IEnumerator valueEnumerator = values.GetEnumerator();

            writer.WriteStartObject();
            writer.WritePropertyName("TableNamespace");
            writer.WriteValue(realVal.TableNamespace?.Value);
            writer.WritePropertyName("HasGameplayTags");
            writer.WriteValue(realVal.HasGameplayTags);
            writer.WritePropertyName("Value");
            writer.WriteStartArray();
            foreach (object key in keys)
            {
                valueEnumerator.MoveNext();

                writer.WriteStartArray();
                serializer.Serialize(writer, key);
                serializer.Serialize(writer, valueEnumerator.Current);
                writer.WriteEndArray();
            }
            writer.WriteEndArray();

            if (realVal.HasGameplayTags)
            {
                writer.WritePropertyName("EntryGameplayTags");
                serializer.Serialize(writer, realVal.EntryGameplayTags);
                writer.WritePropertyName("TrailingTagContainer");
                serializer.Serialize(writer, realVal.TrailingTagContainer);
            }

            writer.WriteEndObject();
        }

        public override bool CanRead
        {
            get { return true; }
        }

        public override object ReadJson(JsonReader reader, Type objectType, object existingValue, JsonSerializer serializer)
        {
            var dictionary = new FStringTable();

            JObject tableJson = JObject.Load(reader);
            dictionary.TableNamespace = new FString(tableJson["TableNamespace"]?.ToObject<string>());

            JToken hasTagsToken = tableJson["HasGameplayTags"];
            dictionary.HasGameplayTags = hasTagsToken != null && hasTagsToken.ToObject<bool>();

            JArray tokens = (JArray)tableJson["Value"];
            foreach (var eachToken in tokens)
            {
                FString key = eachToken[0].ToObject<FString>(serializer);
                FString value = eachToken[1].ToObject<FString>(serializer);
                if (key == null) continue;
                if (dictionary.ContainsKey(key))
                    dictionary[key] = value;
                else
                    dictionary.Add(key, value);
            }

            if (dictionary.HasGameplayTags)
            {
                JToken entryTagsToken = tableJson["EntryGameplayTags"];
                if (entryTagsToken != null)
                    dictionary.EntryGameplayTags = entryTagsToken.ToObject<List<FGameplayTagContainer>>(serializer);

                JToken trailingToken = tableJson["TrailingTagContainer"];
                if (trailingToken != null)
                    dictionary.TrailingTagContainer = trailingToken.ToObject<FGameplayTagContainer>(serializer);
            }

            return dictionary;
        }
    }
}