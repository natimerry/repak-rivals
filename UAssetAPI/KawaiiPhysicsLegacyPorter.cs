using System;
using System.Collections.Generic;
using UAssetAPI.ExportTypes;
using UAssetAPI.PropertyTypes.Objects;
using UAssetAPI.PropertyTypes.Structs;
using UAssetAPI.UnrealTypes;

namespace UAssetAPI;

public sealed class KawaiiPhysicsLegacyPortResult
{
    public int VisitedAnimNodes { get; internal set; }
    public int PortedAnimNodes { get; internal set; }
    public int SkippedExistingChains { get; internal set; }
}

public sealed class KawaiiPhysicsPortOptions
{
    public bool ForceRebuildChain0 { get; set; }
    public bool? UseCurves { get; set; }
    public float? WorldDampingLocation { get; set; }
    public float? WorldDampingRotation { get; set; }
    public float? Stiffness { get; set; }
    public float? Damping { get; set; }
    public float? GravityScale { get; set; }
    public string SimulationSpace { get; set; }
    public float? TeleportDistanceThreshold { get; set; }
    public float? TeleportRotationThreshold { get; set; }
    public bool? EnableWarmUp { get; set; }
    public int? WarmUpFrames { get; set; }
    public bool ClearCurveData { get; set; } = false;
    public bool ClearExternalForces { get; set; } = false;
    public bool DisableWind { get; set; } = false;
    public bool? UseWorldSpaceGravity { get; set; }
    public bool? UseProjectGravity { get; set; }
    public FVector? GravityVector { get; set; }
}

public static class KawaiiPhysicsLegacyPorter
{
    private const string AnimNodeStruct = "AnimNode_KawaiiPhysics";
    private const string ChainStruct = "KawaiiPhysicsChain";
    private const string BoneSettingsStruct = "BoneSettings";
    private const string BoneChainPhysicsSettingsStruct = "BoneChainPhysicsSettings";
    private const string KawaiiPhysicsSettingsStruct = "KawaiiPhysicsSettings";
    private const string BoneConstraintSettingsStruct = "BoneConstraintSettings";
    private const string ExternalForceSettingsStruct = "ExternalForceSettings";
    private const string WaveAnimSettingsStruct = "WaveAnimSettings";


    public static KawaiiPhysicsLegacyPortResult PortLegacyAnimNodes(UAsset asset, bool forceRebuildChain0 = false)
    {
        return PortLegacyAnimNodes(asset, new KawaiiPhysicsPortOptions { ForceRebuildChain0 = forceRebuildChain0 });
    }

    public static KawaiiPhysicsLegacyPortResult PortLegacyAnimNodes(UAsset asset, KawaiiPhysicsPortOptions options)
    {
        if (asset == null) throw new ArgumentNullException(nameof(asset));
        options ??= new KawaiiPhysicsPortOptions();

        var result = new KawaiiPhysicsLegacyPortResult();
        if (asset.Exports == null) return result;

        foreach (Export export in asset.Exports)
        {
            if (export is not NormalExport normalExport || normalExport.Data == null) continue;

            bool mutated = PortPropertyList(asset, normalExport.Data, options, result);
            if (mutated)
            {
                normalExport.OriginalUnversionedHeader = null;
                normalExport.ResolveAncestries(asset, new AncestryInfo());
            }
        }

        return result;
    }

    private static bool PortPropertyList(UAsset asset, List<PropertyData> properties, KawaiiPhysicsPortOptions options, KawaiiPhysicsLegacyPortResult result)
    {
        bool mutated = false;
        for (int i = 0; i < properties.Count; i++)
        {
            mutated |= PortProperty(asset, properties[i], options, result);
        }

        return mutated;
    }

    private static bool PortProperty(UAsset asset, PropertyData property, KawaiiPhysicsPortOptions options, KawaiiPhysicsLegacyPortResult result)
    {
        switch (property)
        {
            case StructPropertyData structProperty:
                bool mutated = false;
                if (IsKawaiiAnimNode(structProperty))
                {
                    result.VisitedAnimNodes++;
                    mutated |= PortAnimNode(asset, structProperty, options, result);
                }

                if (structProperty.Value != null)
                {
                    mutated |= PortPropertyList(asset, structProperty.Value, options, result);
                }

                return mutated;

            case ArrayPropertyData arrayProperty when arrayProperty.Value != null:
                bool arrayMutated = false;
                foreach (PropertyData item in arrayProperty.Value)
                {
                    arrayMutated |= PortProperty(asset, item, options, result);
                }

                return arrayMutated;

            default:
                return false;
        }
    }

    private static bool PortAnimNode(UAsset asset, StructPropertyData node, KawaiiPhysicsPortOptions options, KawaiiPhysicsLegacyPortResult result)
    {
        ArrayPropertyData chains = Get<ArrayPropertyData>(node, "Chains");
        bool hasChains = chains?.Value != null && chains.Value.Length > 0;
        if (hasChains && !options.ForceRebuildChain0)
        {
            result.SkippedExistingChains++;
            return false;
        }

        if (!HasLegacyKawaiiSettings(node) && !hasChains) return false;

        StructPropertyData chain = HasLegacyKawaiiSettings(node)
            ? BuildChainFromLegacyNode(asset, node, options)
            : RebuildExistingChain(asset, chains.Value[0], options);

        if (chain == null) return false;
        if (chains == null)
        {
            chains = new ArrayPropertyData(Name(asset, "Chains"));
            Set(node, "Chains", chains);
        }

        chains.ArrayType = Name(asset, "StructProperty");
        chains.DummyStruct = chain;
        chains.Value = options.ForceRebuildChain0 && hasChains ? RebuildFirstChain(chains.Value, chain) : new PropertyData[] { chain };
        ApplyNodeStartupStabilization(asset, node, options);
        node._originalStructHeader = null;
        result.PortedAnimNodes++;
        return true;
    }

    private static PropertyData[] RebuildFirstChain(PropertyData[] existingChains, StructPropertyData replacement)
    {
        if (existingChains == null || existingChains.Length == 0) return new PropertyData[] { replacement };

        PropertyData[] rebuilt = new PropertyData[existingChains.Length];
        rebuilt[0] = replacement;
        for (int i = 1; i < existingChains.Length; i++)
        {
            rebuilt[i] = existingChains[i];
        }

        return rebuilt;
    }

    private static StructPropertyData RebuildExistingChain(UAsset asset, PropertyData existingChain, KawaiiPhysicsPortOptions options)
    {
        if (existingChain is not StructPropertyData chain) return null;

        StructPropertyData rebuilt = (StructPropertyData)DeepClone(chain);
        rebuilt.Name = Name(asset, "Chains");
        rebuilt.StructType = Name(asset, ChainStruct);
        ClearCachedUnversionedHeaders(rebuilt);

        NormalizeChainForRivals(asset, rebuilt, options);

        return rebuilt;
    }

    private static void NormalizeChainForRivals(UAsset asset, StructPropertyData chain, KawaiiPhysicsPortOptions options)
    {
        StructPropertyData chainPhysics = EnsureStruct(asset, chain, "PhysicsSettings", BoneChainPhysicsSettingsStruct);
        StructPropertyData physicsSettings = EnsureStruct(asset, chainPhysics, "PhysicsSettings", KawaiiPhysicsSettingsStruct);
        ApplyPhysicsDefaults(asset, physicsSettings, options);
        ApplyChainStartupStabilization(asset, chainPhysics, options);
        ApplyCurveOptions(chainPhysics, options);
        if (options.UseCurves.HasValue)
        {
            Set(chainPhysics, "bUseCurve", Bool(asset, "bUseCurve", options.UseCurves.Value));
        }

        StructPropertyData externalForceSettings = EnsureStruct(asset, chain, "ExternalForceSettings", ExternalForceSettingsStruct);
        ApplyExternalForceOptions(asset, externalForceSettings, options);
    }

    private static StructPropertyData ClonePhysicsSettings(UAsset asset, StructPropertyData node)
    {
        PropertyData cloned = CloneAs(asset, node, "PhysicsSettings", "PhysicsSettings");
        if (cloned is not StructPropertyData physStruct) return null;

        physStruct.Value ??= new List<PropertyData>();

        return physStruct;
    }

    private static void ApplyPhysicsDefaults(UAsset asset, StructPropertyData physStruct, KawaiiPhysicsPortOptions options)
    {
        physStruct.Value ??= new List<PropertyData>();

        if (options.WorldDampingLocation.HasValue) SetFloat(asset, physStruct, "WorldDampingLocation", Math.Max(0.0f, options.WorldDampingLocation.Value));
        if (options.WorldDampingRotation.HasValue) SetFloat(asset, physStruct, "WorldDampingRotation", Math.Max(0.0f, options.WorldDampingRotation.Value));
        if (options.Stiffness.HasValue) SetFloat(asset, physStruct, "Stiffness", Math.Clamp(options.Stiffness.Value, 0.0f, 1.0f));
        if (options.Damping.HasValue) SetFloat(asset, physStruct, "Damping", Math.Clamp(options.Damping.Value, 0.0f, 1.0f));
        if (options.GravityScale.HasValue) SetFloat(asset, physStruct, "GravityScale", Math.Max(0.0f, options.GravityScale.Value));
    }

    private static void ApplyNodeStartupStabilization(UAsset asset, StructPropertyData node, KawaiiPhysicsPortOptions options)
    {
        if (!string.IsNullOrWhiteSpace(options.SimulationSpace))
        {
            SetEnumName(asset, node, "SimulationSpace", "EKawaiiPhysicsSimulationSpace", options.SimulationSpace);
        }

        if (options.EnableWarmUp.HasValue)
        {
            SetBool(asset, node, "bNeedWarmUp", options.EnableWarmUp.Value);
            if (options.EnableWarmUp.Value)
            {
                SetBool(asset, node, "bUseWarmUpWhenResetDynamics", true);
            }
        }

        if (options.WarmUpFrames.HasValue)
        {
            SetBool(asset, node, "bNeedWarmUp", true);
            SetBool(asset, node, "bUseWarmUpWhenResetDynamics", true);
            SetInt(asset, node, "WarmUpFrames", Math.Max(0, options.WarmUpFrames.Value));
        }

        if (options.UseWorldSpaceGravity.HasValue) SetBool(asset, node, "bUseWorldSpaceGravity", options.UseWorldSpaceGravity.Value);
        if (options.UseProjectGravity.HasValue) SetBool(asset, node, "bUseDefaultGravityZProjectSetting", options.UseProjectGravity.Value);
    }

    private static void ApplyChainStartupStabilization(UAsset asset, StructPropertyData chainPhysics, KawaiiPhysicsPortOptions options)
    {
        if (options.TeleportDistanceThreshold.HasValue) SetFloat(asset, chainPhysics, "TeleportDistanceThreshold", Math.Max(0.0f, options.TeleportDistanceThreshold.Value));
        if (options.TeleportRotationThreshold.HasValue) SetFloat(asset, chainPhysics, "TeleportRotationThreshold", Math.Max(0.0f, options.TeleportRotationThreshold.Value));
    }

    private static void ApplyCurveOptions(StructPropertyData chainPhysics, KawaiiPhysicsPortOptions options)
    {
        if (!options.ClearCurveData) return;

        Remove(chainPhysics,
            "LimitLinearCurveData",
            "GravityCurveData",
            "DampingCurveData",
            "StiffnessCurveData",
            "WorldDampingLocationCurveData",
            "WorldDampingRotationCurveData",
            "RadiusCurveData",
            "LimitAngleCurveData");
    }

    private static void ApplyExternalForceOptions(UAsset asset, StructPropertyData externalForceSettings, KawaiiPhysicsPortOptions options)
    {
        if (options.GravityVector.HasValue)
        {
            SetVector(asset, externalForceSettings, "Gravity", options.GravityVector.Value);
        }

        if (options.DisableWind)
        {
            SetBool(asset, externalForceSettings, "bEnableWind", false);
            SetFloat(asset, externalForceSettings, "WindScale", 0.0f);
        }

        if (options.ClearExternalForces)
        {
            SetEmptyArray(asset, externalForceSettings, "ExternalForces", "StructProperty", "InstancedStruct");
            SetEmptyArray(asset, externalForceSettings, "CustomExternalForces", "ObjectProperty");
        }
    }

    private static StructPropertyData BuildChainFromLegacyNode(UAsset asset, StructPropertyData node, KawaiiPhysicsPortOptions options)
    {
        var chain = Struct(asset, "Chains", ChainStruct);

        Set(chain, "BoneSettings", Struct(asset, "BoneSettings", BoneSettingsStruct,
            CloneAs(asset, node, "RootBone", "RootBone"),
            CloneAs(asset, node, "ExcludeBones", "ExcludeBones"),
            CloneAs(asset, node, "bRootBoneSimulate", "bRootBoneSimulate"),
            CloneAs(asset, node, "bShouldFixTailBone", "bShouldFixTailBone"),
            CloneAs(asset, node, "FixedBone", "FixedBone"),
            CloneAs(asset, node, "FollowBone", "FollowBone"),
            CloneAs(asset, node, "AdditionalRootBones", "AdditionalRootBones"),
            CloneAs(asset, node, "DummyBoneLength", "DummyBoneLength"),
            CloneAs(asset, node, "BoneForwardAxis", "BoneForwardAxis")));

        var chainPhysicsSettings = Struct(asset, "PhysicsSettings", BoneChainPhysicsSettingsStruct,
            ClonePhysicsSettings(asset, node),
            CloneAs(asset, node, "CustomShapes", "CustomShapes"),
            options.UseCurves.HasValue ? Bool(asset, "bUseCurve", options.UseCurves.Value) : CloneAs(asset, node, "bUseCurve", "bUseCurve"),
            CloneAs(asset, node, "PhysicsParamsPerBone", "PhysicsParamsPerBone"),
            CloneAs(asset, node, "TeleportDistanceThreshold", "TeleportDistanceThreshold"),
            CloneAs(asset, node, "TeleportRotationThreshold", "TeleportRotationThreshold"),
            CloneAs(asset, node, "PlanarConstraint", "PlanarConstraint"),
            CloneAs(asset, node, "LimitLinearCurveData", "LimitLinearCurveData"),
            CloneAs(asset, node, "GravityCurveData", "GravityCurveData"),
            CloneAs(asset, node, "DampingCurveData", "DampingCurveData"),
            CloneAs(asset, node, "StiffnessCurveData", "StiffnessCurveData"),
            CloneAs(asset, node, "WorldDampingLocationCurveData", "WorldDampingLocationCurveData"),
            CloneAs(asset, node, "WorldDampingRotationCurveData", "WorldDampingRotationCurveData"),
            CloneAs(asset, node, "RadiusCurveData", "RadiusCurveData"),
            CloneAs(asset, node, "LimitAngleCurveData", "LimitAngleCurveData"));
        Set(chain, "PhysicsSettings", chainPhysicsSettings);

        Set(chain, "BoneConstraintSettings", Struct(asset, "BoneConstraintSettings", BoneConstraintSettingsStruct,
            CloneAs(asset, node, "BoneConstraintGlobalComplianceType", "BoneConstraintGlobalComplianceType"),
            CloneAs(asset, node, "BoneConstraintIterationCountBeforeCollision", "BoneConstraintIterationCountBeforeCollision"),
            CloneAs(asset, node, "BoneConstraintIterationCountAfterCollision", "BoneConstraintIterationCountAfterCollision"),
            CloneAs(asset, node, "bAutoAddChildDummyBoneConstraint", "bAutoAddChildDummyBoneConstraint"),
            CloneAs(asset, node, "BoneConstraints", "BoneConstraints"),
            CloneAs(asset, node, "BoneConstraintsDataAsset", "BoneConstraintsDataAsset"),
            CloneAs(asset, node, "BoneConstraintsData", "BoneConstraintsData")));

        var externalForceSettings = Struct(asset, "ExternalForceSettings", ExternalForceSettingsStruct,
            CloneAs(asset, node, "bDisableAllExternalForces", "bDisableAllExternalForces"),
            CloneAs(asset, node, "Gravity", "Gravity"),    
            CloneAs(asset, node, "bEnableWind", "bEnableWind"),
            CloneAs(asset, node, "WindScale", "WindScale"),
            CloneAs(asset, node, "ExternalForces", "ExternalForces"),
            CloneAs(asset, node, "CustomExternalForces", "CustomExternalForces"));
        Set(chain, "ExternalForceSettings", externalForceSettings);

        Set(chain, "WaveAnimSettings", Struct(asset, "WaveAnimSettings", WaveAnimSettingsStruct,
            CloneAs(asset, node, "bEnableWaveAnim", "bEnableWaveAnim"),
            CloneAs(asset, node, "WaveBeginBone", "WaveBeginBone"),
            CloneAs(asset, node, "WaveFrequncy", "WaveFrequncy"),
            CloneAs(asset, node, "WaveFrequency", "WaveFrequncy"),
            CloneAs(asset, node, "WaveNum", "WaveNum"),
            CloneAs(asset, node, "WaveDirection", "WaveDirection"),
            CloneAs(asset, node, "WaveAmplitude", "WaveAmplitude"),
            CloneAs(asset, node, "WaveAmplitudeCurveData", "WaveAmplitudeCurveData")));
        Set(chain, "AutoConfiguredLODThreshold", Int(asset, "AutoConfiguredLODThreshold", -1));
        NormalizeChainForRivals(asset, chain, options);

        return chain;
    }

    private static bool IsKawaiiAnimNode(StructPropertyData property)
    {
        string structType = property.StructType?.Value?.Value;
        if (string.Equals(structType, AnimNodeStruct, StringComparison.Ordinal) ||
            string.Equals(structType, "F" + AnimNodeStruct, StringComparison.Ordinal))
        {
            return true;
        }

        return string.Equals(property.Name?.Value?.Value, "Node", StringComparison.Ordinal) &&
               Get(property, "RootBone") != null &&
               Get(property, "PhysicsSettings") != null;
    }

    private static bool HasLegacyKawaiiSettings(StructPropertyData node)
    {
        return Get(node, "RootBone") != null ||
               Get(node, "PhysicsSettings") != null ||
               Get(node, "ExternalForces") != null ||
               Get(node, "BoneConstraints") != null ||
               Get(node, "CustomShapes") != null ||
               Get(node, "PhysicsParamsPerBone") != null ||
               Get(node, "LimitsDataAsset") != null;
    }

    private static StructPropertyData Struct(UAsset asset, string name, string structType, params PropertyData[] properties)
    {
        var result = new StructPropertyData(Name(asset, name), Name(asset, structType))
        {
            StructGUID = Guid.Empty,
            SerializeNone = true,
            Value = new List<PropertyData>()
        };

        foreach (PropertyData property in properties)
        {
            if (property != null) result.Value.Add(property);
        }

        return result;
    }

    private static BoolPropertyData Bool(UAsset asset, string name, bool value)
    {
        return new BoolPropertyData(Name(asset, name)) { Value = value };
    }

    private static IntPropertyData Int(UAsset asset, string name, int value)
    {
        return new IntPropertyData(Name(asset, name)) { Value = value };
    }

    private static FloatPropertyData Float(UAsset asset, string name, float value)
    {
        return new FloatPropertyData(Name(asset, name)) { Value = value };
    }

    private static StructPropertyData EnsureStruct(UAsset asset, StructPropertyData parent, string name, string structType)
    {
        if (Get(parent, name) is StructPropertyData existing)
        {
            existing.Name = Name(asset, name);
            existing.StructType = Name(asset, structType);
            existing.Value ??= new List<PropertyData>();
            existing.StructGUID = Guid.Empty;
            existing.SerializeNone = true;
            return existing;
        }

        StructPropertyData created = Struct(asset, name, structType);
        Set(parent, name, created);
        return created;
    }

    private static void SetBool(UAsset asset, StructPropertyData property, string name, bool value)
    {
        if (Get(property, name) is BoolPropertyData existing)
        {
            existing.Value = value;
            return;
        }

        Set(property, name, Bool(asset, name, value));
    }

    private static void SetInt(UAsset asset, StructPropertyData property, string name, int value)
    {
        if (Get(property, name) is IntPropertyData existing)
        {
            existing.Value = value;
            return;
        }

        Set(property, name, Int(asset, name, value));
    }

    private static void SetFloat(UAsset asset, StructPropertyData property, string name, float value)
    {
        if (Get(property, name) is FloatPropertyData existing)
        {
            existing.Value = value;
            return;
        }

        Set(property, name, Float(asset, name, value));
    }

    private static void SetVector(UAsset asset, StructPropertyData property, string name, FVector value)
    {
        if (Get(property, name) is VectorPropertyData existing)
        {
            existing.Value = value;
            return;
        }

        Set(property, name, new VectorPropertyData(Name(asset, name)) { Value = value });
    }

    private static void SetEmptyArray(UAsset asset, StructPropertyData property, string name, string arrayType, string structType = null)
    {
        if (Get(property, name) is ArrayPropertyData existing)
        {
            existing.ArrayType ??= Name(asset, arrayType);
            existing.Value = Array.Empty<PropertyData>();
            if (arrayType == "StructProperty" && existing.DummyStruct == null && structType != null)
            {
                existing.DummyStruct = new StructPropertyData(Name(asset, name), Name(asset, structType))
                {
                    StructGUID = Guid.Empty
                };
            }
            return;
        }

        var emptyArray = new ArrayPropertyData(Name(asset, name))
        {
            ArrayType = Name(asset, arrayType),
            Value = Array.Empty<PropertyData>()
        };

        if (arrayType == "StructProperty" && structType != null)
        {
            emptyArray.DummyStruct = new StructPropertyData(Name(asset, name), Name(asset, structType))
            {
                StructGUID = Guid.Empty
            };
        }

        Set(property, name, emptyArray);
    }

    private static void SetEnumName(UAsset asset, StructPropertyData property, string name, string enumType, string value)
    {
        string normalizedValue = NormalizeEnumValue(enumType, value);
        PropertyData existing = Get(property, name);

        if (existing is EnumPropertyData enumProperty)
        {
            enumProperty.EnumType ??= EnumName(asset, enumType);
            enumProperty.InnerType ??= EnumName(asset, "ByteProperty");
            enumProperty.Value = EnumName(asset, normalizedValue);
            return;
        }

        if (existing is BytePropertyData byteProperty)
        {
            byteProperty.ByteType = BytePropertyType.FName;
            byteProperty.EnumType = EnumName(asset, enumType);
            byteProperty.EnumValue = EnumName(asset, normalizedValue);
            return;
        }

        Set(property, name, new EnumPropertyData(Name(asset, name))
        {
            EnumType = EnumName(asset, enumType),
            InnerType = EnumName(asset, "ByteProperty"),
            Value = EnumName(asset, normalizedValue)
        });
    }

    private static string NormalizeEnumValue(string enumType, string value)
    {
        string trimmed = value.Trim();
        if (trimmed.Contains("::", StringComparison.Ordinal)) return trimmed;
        return enumType + "::" + trimmed;
    }

    private static FName EnumName(UAsset asset, string value)
    {
        return asset.HasUnversionedProperties ? FName.DefineDummy(asset, value) : FName.FromString(asset, value);
    }

    private static PropertyData CloneAs(UAsset asset, StructPropertyData source, string oldName, string newName)
    {
        PropertyData original = Get(source, oldName);
        if (original == null) return null;

        PropertyData clone = DeepClone(original);
        clone.Name = Name(asset, newName);
        ClearCachedUnversionedHeaders(clone);
        return clone;
    }

    private static PropertyData DeepClone(PropertyData property)
    {
        PropertyData clone = (PropertyData)property.Clone();

        if (property is ArrayPropertyData sourceArray && clone is ArrayPropertyData clonedArray)
        {
            if (sourceArray.Value != null)
            {
                var clonedValues = new PropertyData[sourceArray.Value.Length];
                for (int i = 0; i < sourceArray.Value.Length; i++)
                {
                    clonedValues[i] = sourceArray.Value[i] == null ? null : DeepClone(sourceArray.Value[i]);
                }

                clonedArray.Value = clonedValues;
            }

            clonedArray.DummyStruct = sourceArray.DummyStruct == null ? null : (StructPropertyData)DeepClone(sourceArray.DummyStruct);
        }

        return clone;
    }

    private static void ClearCachedUnversionedHeaders(PropertyData property)
    {
        switch (property)
        {
            case StructPropertyData structProperty:
                structProperty._originalStructHeader = null;
                if (structProperty.Value == null) return;
                foreach (PropertyData child in structProperty.Value) ClearCachedUnversionedHeaders(child);
                break;

            case ArrayPropertyData arrayProperty:
                if (arrayProperty.DummyStruct != null) ClearCachedUnversionedHeaders(arrayProperty.DummyStruct);
                if (arrayProperty.Value == null) return;
                foreach (PropertyData child in arrayProperty.Value) ClearCachedUnversionedHeaders(child);
                break;
        }
    }

    private static PropertyData Get(StructPropertyData property, string name)
    {
        if (property.Value == null) return null;

        foreach (PropertyData child in property.Value)
        {
            if (string.Equals(child?.Name?.Value?.Value, name, StringComparison.Ordinal)) return child;
        }

        return null;
    }

    private static T Get<T>(StructPropertyData property, string name) where T : PropertyData
    {
        return Get(property, name) as T;
    }

    private static void Set(StructPropertyData property, string name, PropertyData value)
    {
        if (value == null) return;
        value.Name = Name(property.Name?.Asset as UAsset, name);

        property.Value ??= new List<PropertyData>();
        for (int i = 0; i < property.Value.Count; i++)
        {
            if (string.Equals(property.Value[i]?.Name?.Value?.Value, name, StringComparison.Ordinal))
            {
                property.Value[i] = value;
                return;
            }
        }

        property.Value.Add(value);
    }

    private static void Remove(StructPropertyData property, params string[] names)
    {
        if (property.Value == null || names == null || names.Length == 0) return;

        var removeNames = new HashSet<string>(names, StringComparer.Ordinal);
        property.Value.RemoveAll(child => child?.Name?.Value?.Value != null && removeNames.Contains(child.Name.Value.Value));
    }

    private static FName Name(UAsset asset, string value)
    {
        return new FName(asset, value);
    }
}
