/*
    This code has been slightly adapted from code written by @trumank as part of the repak crate: https://github.com/trumank/repak

    MIT License

    Copyright 2024 Truman Kilen, spuds

    Permission is hereby granted, free of charge, to any person obtaining a copy
    of this software and associated documentation files (the "Software"), to deal
    in the Software without restriction, including without limitation the rights
    to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
    copies of the Software, and to permit persons to whom the Software is
    furnished to do so, subject to the following conditions:
    The above copyright notice and this permission notice shall be included in all
    copies or substantial portions of the Software.
    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
    OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
    SOFTWARE.
*/

namespace UAssetAPI;

using Microsoft.Win32.SafeHandles;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using UAssetAPI.PropertyTypes.Objects;

public enum PakVersion : byte
{
    V0 = 0,
    V1 = 1,
    V2 = 2,
    V3 = 3,
    V4 = 4,
    V5 = 5,
    V6 = 6,
    V7 = 7,
    V8A = 8,
    V8B = 9,
    V9 = 10,
    V10 = 11,
    V11 = 12
}

public enum PakCompression : byte
{
    Zlib,
    Gzip,
    Oodle,
    Zstd
}

public class PakBuilder : SafeHandleZeroOrMinusOneIsInvalid
{
    private static bool _resolverRegistered = false;

    /// <summary>
    /// Get the platform-specific native library filename for repak.
    /// </summary>
    private static string GetPlatformLibraryName()
    {
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            return "repak_bind.dll";
        else if (RuntimeInformation.IsOSPlatform(OSPlatform.Linux))
            return "librepak_bind.so";
        else if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
            return "librepak_bind.dylib";
        return "repak_bind.dll";
    }

    /// <summary>
    /// Register a DllImport resolver so the runtime finds the correct repak library per platform.
    /// </summary>
    private static void RegisterResolver()
    {
        if (_resolverRegistered) return;
        _resolverRegistered = true;

        NativeLibrary.SetDllImportResolver(typeof(RePakInterop).Assembly, (libraryName, assembly, searchPath) =>
        {
            if (libraryName == RePakInterop.NativeLib)
            {
                string platformLib = GetPlatformLibraryName();
                // Try beside the executable first
                string fullPath = Path.Combine(AppContext.BaseDirectory, platformLib);
                if (NativeLibrary.TryLoad(fullPath, out IntPtr handle))
                    return handle;
                // Try current directory
                if (NativeLibrary.TryLoad(platformLib, assembly, searchPath, out handle))
                    return handle;
            }
            return IntPtr.Zero;
        });
    }

    public PakBuilder() : base(true)
    {
        RegisterResolver();

        try
        {
            SetHandle(RePakInterop.pak_builder_new());
        }
        catch (Exception ex)
        {
            if (ex is DllNotFoundException || ex is BadImageFormatException)
            {
                // Extract embedded native lib for the current platform
                string libName = GetPlatformLibraryName();
                string embeddedResourceName = "UAssetAPI.repak_bind.dll"; // embedded resource is always the Windows DLL

                // On non-Windows, the embedded Windows DLL won't work, so give a clear error
                if (!RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
                {
                    throw new DllNotFoundException(
                        $"repak native library not found: {libName}. " +
                        $"Place the platform-specific repak library ({libName}) beside the UAssetTool executable. " +
                        $"Build repak for your platform from https://github.com/trumank/repak");
                }

                // Windows: extract embedded DLL
                using (var resource = typeof(PropertyData).Assembly.GetManifestResourceStream(embeddedResourceName))
                {
                    if (resource != null)
                    {
                        using (var file = new FileStream(libName, FileMode.Create, FileAccess.Write))
                        {
                            resource.CopyTo(file);
                        }
                    }
                }

                SetHandle(RePakInterop.pak_builder_new());
            }
            else
            {
                throw;
            }
        }
    }
    protected override bool ReleaseHandle()
    {
        RePakInterop.pak_builder_drop(handle);
        return true;
    }

    public PakBuilder Key(byte[] key)
    {
        if (key.Length != 32) throw new Exception("Invalid AES key length");
        SetHandle(RePakInterop.pak_builder_key(handle, key));
        return this;
    }

    public PakBuilder Compression(PakCompression[] compressions)
    {
        SetHandle(RePakInterop.pak_builder_compression(handle, compressions.Select(c => Convert.ToByte(c)).ToArray(), compressions.Length));
        return this;
    }

    public PakWriter Writer(Stream stream, PakVersion version = PakVersion.V11, string mountPoint = "../../../", ulong pathHashSeed = 0)
    {
        if (handle == IntPtr.Zero) throw new Exception("PakBuilder handle invalid");

        var streamCtx = StreamCallbacks.Create(stream);

        IntPtr writerHandle = RePakInterop.pak_builder_writer(handle, streamCtx, version, mountPoint, pathHashSeed);

        // pak_builder_reader consumes the builder
        SetHandleAsInvalid();
        SetHandle(IntPtr.Zero);

        return new PakWriter(writerHandle, streamCtx.Context);
    }

    public PakReader Reader(Stream stream)
    {
        if (handle == IntPtr.Zero) throw new Exception("PakBuilder handle invalid");

        var streamCtx = StreamCallbacks.Create(stream);

        IntPtr readerHandle = RePakInterop.pak_builder_reader(handle, streamCtx);
        StreamCallbacks.Free(streamCtx.Context);

        // pak_builder_reader consumes the builder
        SetHandleAsInvalid();
        SetHandle(IntPtr.Zero);

        if (readerHandle == IntPtr.Zero) throw new Exception("Failed to create PakReader");
        return new PakReader(readerHandle, stream);
    }
}

public class PakWriter : SafeHandleZeroOrMinusOneIsInvalid
{
    private IntPtr _streamCtx;

    public PakWriter(IntPtr handle, IntPtr streamCtx) : base(true)
    {
        SetHandle(handle);
        _streamCtx = streamCtx;
    }
    protected override bool ReleaseHandle()
    {
        RePakInterop.pak_writer_drop(handle);
        StreamCallbacks.Free(_streamCtx);
        return true;
    }

    public void WriteFile(string path, byte[] data)
    {
        if (handle == IntPtr.Zero) throw new Exception("PakWriter handle invalid");
        int result = RePakInterop.pak_writer_write_file(handle, path, data, data.Length);
        if (result != 0) throw new Exception("Failed to write file");
    }

    public void WriteIndex()
    {
        int result = RePakInterop.pak_writer_write_index(handle);

        // write_index drops the writer
        SetHandleAsInvalid();
        SetHandle(IntPtr.Zero);
        StreamCallbacks.Free(_streamCtx);

        if (result != 0) throw new Exception("Failed to write index");
    }
}

public class PakReader : SafeHandleZeroOrMinusOneIsInvalid
{
    public PakReader(IntPtr handle, Stream stream) : base(true)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        RePakInterop.pak_reader_drop(handle);
        return true;
    }

    public string GetMountPoint()
    {
        if (handle == IntPtr.Zero) throw new Exception("PakReader handle invalid");

        var cstring = RePakInterop.pak_reader_mount_point(handle);
        var mountPoint = Marshal.PtrToStringAnsi(cstring);

        RePakInterop.pak_cstring_drop(cstring);

        return mountPoint;
    }

    public PakVersion GetVersion()
    {
        if (handle == IntPtr.Zero) throw new Exception("PakReader handle invalid");

        return RePakInterop.pak_reader_version(handle);
    }

    public byte[] Get(Stream stream, string path)
    {
        if (handle == IntPtr.Zero) throw new Exception("PakReader handle invalid");

        var streamCtx = StreamCallbacks.Create(stream);

        IntPtr bufferPtr;
        ulong length;
        int result = RePakInterop.pak_reader_get(handle, path, streamCtx, out bufferPtr, out length);

        StreamCallbacks.Free(streamCtx.Context);

        if (result != 0) return null;

        byte[] buffer = new byte[length];
        Marshal.Copy(bufferPtr, buffer, 0, (int)length);

        RePakInterop.pak_buffer_drop(bufferPtr, length);

        return buffer;
    }

    public string[] Files()
    {
        if (handle == IntPtr.Zero) throw new Exception("PakReader handle invalid");

        ulong length;
        IntPtr filesPtr = RePakInterop.pak_reader_files(handle, out length);
        var files = new List<string>();
        for (ulong i = 0; i < length; i++)
        {
            IntPtr currentPtr = Marshal.ReadIntPtr(filesPtr, (int)i * IntPtr.Size);
            files.Add(Marshal.PtrToStringAnsi(currentPtr));
        }
        RePakInterop.pak_drop_files(filesPtr, length);
        return files.ToArray();
    }
}


public static class StreamCallbacks
{
    public static RePakInterop.StreamCallbacks Create(Stream stream)
    {
        return new RePakInterop.StreamCallbacks
        {
            Context = GCHandle.ToIntPtr(GCHandle.Alloc(stream)),
            ReadCb = StreamCallbacks.ReadCallback,
            WriteCb = StreamCallbacks.WriteCallback,
            SeekCb = StreamCallbacks.SeekCallback,
            FlushCb = StreamCallbacks.FlushCallback
        };
    }
    public static void Free(IntPtr streamCtx)
    {
        GCHandle.FromIntPtr(streamCtx).Free();
    }

    public static long ReadCallback(IntPtr context, IntPtr buffer, ulong bufferLen)
    {
        var stream = (Stream)GCHandle.FromIntPtr(context).Target;
        try
        {
            byte[] bufferManaged = new byte[bufferLen];
            int bytesRead = stream.Read(bufferManaged, 0, (int)bufferLen);
            Marshal.Copy(bufferManaged, 0, buffer, bytesRead);
            return bytesRead;
        }
        catch (Exception e)
        {
            Console.WriteLine($"Error during read {e}");
            return -1;
        }
    }

    public static int WriteCallback(IntPtr context, IntPtr buffer, int bufferLen)
    {
        var stream = (Stream)GCHandle.FromIntPtr(context).Target;
        var bufferManaged = new byte[bufferLen];
        Marshal.Copy(buffer, bufferManaged, 0, bufferLen);

        try
        {
            stream.Write(bufferManaged, 0, bufferLen);
            return bufferLen;
        }
        catch (Exception e)
        {
            Console.WriteLine($"Error during write {e}");
            return -1;
        }
    }

    public static ulong SeekCallback(IntPtr context, long offset, int origin)
    {
        var stream = (Stream)GCHandle.FromIntPtr(context).Target;
        try
        {
            long newPosition = stream.Seek(offset, (SeekOrigin)origin);
            return (ulong)newPosition;
        }
        catch (Exception e)
        {
            Console.WriteLine($"Error during seek {e}");
            return ulong.MaxValue;
        }
    }

    public static int FlushCallback(IntPtr context)
    {
        var stream = (Stream)GCHandle.FromIntPtr(context).Target;
        try
        {
            stream.Flush();
            return 0;
        }
        catch (Exception e)
        {
            Console.WriteLine($"Error during flush {e}");
            return 1;
        }
    }
}