@echo off
setlocal
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" exit /b 1
for /f "usebackq tokens=*" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "TASK_VS_ROOT=%%i"
if not defined TASK_VS_ROOT exit /b 1
call "%TASK_VS_ROOT%\VC\Auxiliary\Build\vcvars64.bat" >nul
if errorlevel 1 exit /b 1
set "PATH=%TASK_VS_ROOT%\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin;%TASK_VS_ROOT%\Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja;%PATH%"
set "TASK_CLANG=%ProgramFiles%\LLVM\bin\clang-cl.exe"
if not exist "%TASK_CLANG%" (
    echo Instala LLVM en Program Files\LLVM, o ajusta TASK_CLANG.
    exit /b 1
)
cd /d "%~dp0.."
cmake -S llama-source -B tools/cpu-patched-avx2 -G Ninja -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_SYSTEM_PROCESSOR=AMD64 -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER="%TASK_CLANG%" -DCMAKE_CXX_COMPILER="%TASK_CLANG%" -DBUILD_SHARED_LIBS=ON -DGGML_NATIVE=OFF -DGGML_AVX=ON -DGGML_AVX2=ON -DGGML_OPENMP=OFF -DLLAMA_BUILD_COMMON=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_TOOLS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_MTMD=ON
if errorlevel 1 exit /b 1
cmake --build tools/cpu-patched-avx2 --config Release -j 4 --target llama mtmd
exit /b %errorlevel%
