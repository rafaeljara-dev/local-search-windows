#define NOMINMAX
#include <windows.h>
#include <psapi.h>
#include <chrono>
#include <cmath>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>
#include "llama.h"
#include "mtmd.h"
#include "mtmd-helper.h"
#include "json.hpp"

using json = nlohmann::json;
using Clock = std::chrono::steady_clock;
double ms(Clock::time_point start) { return std::chrono::duration<double, std::milli>(Clock::now()-start).count(); }
json memory() {
    PROCESS_MEMORY_COUNTERS_EX stats{};
    GetProcessMemoryInfo(GetCurrentProcess(), reinterpret_cast<PROCESS_MEMORY_COUNTERS*>(&stats), sizeof(stats));
    return {{"working_set_bytes",stats.WorkingSetSize},{"peak_working_set_bytes",stats.PeakWorkingSetSize},{"private_bytes",stats.PrivateUsage}};
}
FARPROC symbol(HMODULE module, const char* name) {
    auto pointer = GetProcAddress(module,name);
    if (!pointer) throw std::runtime_error(std::string("Missing export: ")+name);
    return pointer;
}
#define BIND(module, name) auto name = reinterpret_cast<decltype(&::name)>(symbol(module,#name))

int main(int argc, char** argv) {
    if(argc<4) { std::cerr << "Usage: embed-worker DLL_DIRECTORY MODEL THREADS [CONTEXT] [MMPROJ]\n"; return 2; }
    try {
        const auto started=Clock::now();
        SetDllDirectoryA(argv[1]);
        auto library=LoadLibraryA((std::string(argv[1])+"\\llama.dll").c_str());
        auto ggml=LoadLibraryA((std::string(argv[1])+"\\ggml.dll").c_str());
        if(!library || !ggml) throw std::runtime_error("Cannot load llama/ggml DLLs");
        BIND(ggml,ggml_backend_load_all_from_path);
        ggml_backend_load_all_from_path(argv[1]);
        BIND(library,llama_backend_init); BIND(library,llama_backend_free);
        BIND(library,llama_model_default_params); BIND(library,llama_context_default_params);
        BIND(library,llama_model_load_from_file); BIND(library,llama_init_from_model);
        BIND(library,llama_model_get_vocab); BIND(library,llama_model_n_embd_out); BIND(library,llama_model_n_embd_inp);
        BIND(library,llama_batch_ext_init); BIND(library,llama_batch_ext_free); BIND(library,llama_batch_ext_clear);
        BIND(library,llama_batch_ext_add_token); BIND(library,llama_batch_ext_add_embd);
        BIND(library,llama_batch_ext_set_pos); BIND(library,llama_batch_ext_set_output_embd);
        BIND(library,llama_tokenize); BIND(library,llama_process); BIND(library,llama_get_embeddings_seq);
        BIND(library,llama_free); BIND(library,llama_model_free);
        llama_backend_init();
        auto modelParams=llama_model_default_params(); modelParams.n_gpu_layers=0;
        auto model=llama_model_load_from_file(argv[2],modelParams);
        if(!model) throw std::runtime_error("Model load failed");
        const int capacity=argc>4 ? std::stoi(argv[4]) : 256;
        auto params=llama_context_default_params();
        params.n_ctx=capacity; params.n_batch=capacity; params.n_ubatch=capacity;
        params.n_seq_max=1; params.kv_unified=true; params.embeddings=true;
        params.pooling_type=LLAMA_POOLING_TYPE_MEAN;
        params.n_threads=std::stoi(argv[3]); params.n_threads_batch=params.n_threads;
        auto context=llama_init_from_model(model,params);
        if(!context) throw std::runtime_error("Context creation failed");
        auto batch=llama_batch_ext_init(context);
        auto vocab=llama_model_get_vocab(model);
        const int dimension=llama_model_n_embd_out(model);
        const int rowWidth=llama_model_n_embd_inp(model);
        auto mediaLibrary=LoadLibraryA((std::string(argv[1])+"\\mtmd.dll").c_str());
        if(!mediaLibrary) throw std::runtime_error("Cannot load mtmd DLL");
        BIND(mediaLibrary,mtmd_context_params_default); BIND(mediaLibrary,mtmd_init_from_file); BIND(mediaLibrary,mtmd_free);
        BIND(mediaLibrary,mtmd_helper_init_opt_default); BIND(mediaLibrary,mtmd_helper_bitmap_init_from_file);
        BIND(mediaLibrary,mtmd_bitmap_free); BIND(mediaLibrary,mtmd_helper_video_free);
        BIND(mediaLibrary,mtmd_input_chunks_init); BIND(mediaLibrary,mtmd_input_chunks_free);
        BIND(mediaLibrary,mtmd_default_marker); BIND(mediaLibrary,mtmd_tokenize);
        BIND(mediaLibrary,mtmd_input_chunks_size); BIND(mediaLibrary,mtmd_input_chunks_get);
        BIND(mediaLibrary,mtmd_input_chunk_get_type); BIND(mediaLibrary,mtmd_input_chunk_get_tokens_text);
        BIND(mediaLibrary,mtmd_encode_chunk); BIND(mediaLibrary,mtmd_get_output_embd); BIND(mediaLibrary,mtmd_input_chunk_get_n_tokens);
        mtmd_context* media=nullptr;
        if(argc>5) {
            auto mp=mtmd_context_params_default(); mp.use_gpu=false; mp.n_threads=params.n_threads;
            mp.warmup=false; mp.image_min_tokens=70; mp.image_max_tokens=280;
            media=mtmd_init_from_file(argv[5],model,mp);
            if(!media) throw std::runtime_error("Multimodal encoder load failed");
        }
        std::cout << json({{"event","ready"},{"load_ms",ms(started)},{"dimension",dimension},{"memory",memory()}}).dump() << std::endl;
        std::string line;
        while(std::getline(std::cin,line)) {
            if(line.empty()) continue;
            auto request=json::parse(line);
            if(request.value("command","")=="exit") break;
            const auto t=Clock::now();
            try {
                llama_batch_ext_clear(batch);
                int position=0;
                auto place=[&](int index) {
                    if(index<0 || position>=capacity) throw std::runtime_error("Context capacity exceeded");
                    llama_pos pos[]={position++,0,0,0};
                    if(!llama_batch_ext_set_pos(batch,index,pos) || !llama_batch_ext_set_output_embd(batch,index,true)) throw std::runtime_error("Cannot place input");
                };
                if(request.contains("text")) {
                    std::string text=request["text"];
                    std::vector<llama_token> tokens(text.size()+16);
                    int count=llama_tokenize(vocab,text.c_str(),static_cast<int>(text.size()),tokens.data(),static_cast<int>(tokens.size()),true,true);
                    if(count<0 || count>capacity) throw std::runtime_error("Token input too long");
                    for(int i=0;i<count;i++) place(llama_batch_ext_add_token(batch,0,tokens[i]));
                } else {
                    if(!media) throw std::runtime_error("No media encoder loaded");
                    std::string path=request.at("path");
                    auto wrapper=mtmd_helper_bitmap_init_from_file(media,path.c_str(),false,mtmd_helper_init_opt_default());
                    if(wrapper.video_ctx) { mtmd_helper_video_free(wrapper.video_ctx); if(wrapper.bitmap) mtmd_bitmap_free(wrapper.bitmap); throw std::runtime_error("Use video frames and audio windows"); }
                    if(!wrapper.bitmap) throw std::runtime_error("Media decode failed");
                    auto chunks=mtmd_input_chunks_init();
                    try {
                        const auto marker=mtmd_default_marker();
                        mtmd_input_text input{marker,strlen(marker),true,true};
                        const mtmd_bitmap* bitmaps[]={wrapper.bitmap};
                        if(mtmd_tokenize(media,chunks,&input,bitmaps,1)!=0) throw std::runtime_error("Media tokenize failed");
                        for(size_t i=0;i<mtmd_input_chunks_size(chunks);i++) {
                            auto chunk=mtmd_input_chunks_get(chunks,i);
                            if(mtmd_input_chunk_get_type(chunk)==MTMD_INPUT_CHUNK_TYPE_TEXT) {
                                size_t count=0; auto tokens=mtmd_input_chunk_get_tokens_text(chunk,&count);
                                for(size_t j=0;j<count;j++) place(llama_batch_ext_add_token(batch,0,tokens[j]));
                            } else {
                                if(mtmd_encode_chunk(media,chunk)!=0) throw std::runtime_error("Media encode failed");
                                auto rows=mtmd_get_output_embd(media);
                                for(size_t j=0;j<mtmd_input_chunk_get_n_tokens(chunk);j++) place(llama_batch_ext_add_embd(batch,0,llama_embd{rows+j*rowWidth,1,static_cast<size_t>(rowWidth)}));
                            }
                        }
                    } catch(...) { mtmd_input_chunks_free(chunks); mtmd_bitmap_free(wrapper.bitmap); throw; }
                    mtmd_input_chunks_free(chunks); mtmd_bitmap_free(wrapper.bitmap);
                }
                if(llama_process(context,LLAMA_PROCESS_TYPE_ENCODE,batch)!=0) throw std::runtime_error("Inference failed");
                auto pooled=llama_get_embeddings_seq(context,0);
                if(!pooled) throw std::runtime_error("No embedding");
                double sum=0; for(int i=0;i<dimension;i++) sum+=double(pooled[i])*pooled[i];
                if(!std::isfinite(sum) || sum<=0) throw std::runtime_error("Invalid embedding norm");
                std::vector<float> vector(dimension); for(int i=0;i<dimension;i++) vector[i]=static_cast<float>(pooled[i]/std::sqrt(sum));
                std::cout << json({{"id",request.value("id","")},{"tokens",position},{"ms",ms(t)},{"vector",vector},{"memory",memory()}}).dump() << std::endl;
            } catch(const std::exception& error) { std::cout << json({{"id",request.value("id","")},{"error",error.what()}}).dump() << std::endl; }
        }
        if(media) mtmd_free(media);
        llama_batch_ext_free(batch); llama_free(context); llama_model_free(model); llama_backend_free();
        return 0;
    } catch(const std::exception& error) { std::cerr << error.what() << std::endl; return 1; }
}
