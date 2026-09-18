
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |component-wasm-async-import
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'component-wasm-async-import.main/main!) (:mode :native) (:reload-fn 'component-wasm-async-import.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'component-wasm-async-import.main
    %{} 'FileEntry
      :defs $ {}
        'HttpBody $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum HttpBody (:empty) (:text 'String) (:bytes 'Buffer)
          :examples $ []
          :schema $ :: 'EnumDef
        'HttpError $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum HttpError (:capability-denied 'String) (:unsupported 'String) (:invalid-request 'String) (:transport 'String) (:response-too-large 'UInt64)
          :examples $ []
          :schema $ :: 'EnumDef
        'HttpHeader $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct HttpHeader (:name 'String) (:value 'Buffer)
          :examples $ []
          :schema $ :: 'StructDef
        'HttpMethod $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum HttpMethod (:get) (:head) (:post) (:put) (:patch) (:delete) (:options)
          :examples $ []
          :schema $ :: 'EnumDef
        'HttpRequest $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct HttpRequest (:method 'component-wasm-async-import.main/HttpMethod) (:url 'String)
            :headers $ :: 'List 'component-wasm-async-import.main/HttpHeader
            :body 'component-wasm-async-import.main/HttpBody
            :max-response-bytes 'UInt64
          :examples $ []
          :schema $ :: 'StructDef
        'HttpResponse $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct HttpResponse (:status 'UInt16)
            :headers $ :: 'List 'component-wasm-async-import.main/HttpHeader
            :body 'component-wasm-async-import.main/HttpBody
          :examples $ []
          :schema $ :: 'StructDef
        'call-host-combine $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-combine (left right count enabled?) (host-combine left right count enabled?)
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true) (:return 'String)
            :args $ [] 'String 'String 'Number 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |keeps-indirect-parameter-contract)
            :code $ quote $ assert= |left:right:3:true |left:right:3:true
            :tags $ #{} :wasm
        'call-host-flag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-flag (flag) (host-flag flag)
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true) (:return 'Bool)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |keeps-async-bool-contract)
            :code $ quote $ assert= true true
            :tags $ #{} :wasm
        'call-host-http-request $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-http-request (request) (host-http-request request)
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'component-wasm-async-import.main/HttpRequest
            :return $ :: 'Result 'component-wasm-async-import.main/HttpResponse 'component-wasm-async-import.main/HttpError
          :tests $ []
            %{} 'TestEntry (:name |keeps-bounded-http-contract)
              :code $ quote $ match (number->uint64 1024)
                (:ok limit)
                  let
                      request $ HttpRequest :method (HttpMethod :get) :url |https://example.test/items :headers ([]) :body (HttpBody :empty) :max-response-bytes limit
                    assert= request request
                (:err message) (raise message)
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |keeps-http-success-shape)
              :code $ quote $ match (number->uint16 200)
                (:ok status)
                  let
                      response $ HttpResponse :status status :headers ([]) :body $ HttpBody :text |hello
                    assert= (%ok response) (%ok response)
                (:err message) (raise message)
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |keeps-http-limit-error-shape)
              :code $ quote $ match (number->uint64 9)
                (:ok observed)
                  assert=
                    %err $ HttpError :response-too-large observed
                    %err $ HttpError :response-too-large observed
                (:err message) (raise message)
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |keeps-http-capability-error-shape)
              :code $ quote $ assert=
                %err $ HttpError :capability-denied "|origin is not granted"
                %err $ HttpError :capability-denied "|origin is not granted"
              :tags $ #{} :wasm
        'call-host-load $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-load (text) (host-load text)
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'String
            :return $ :: 'Result 'String 'String
          :tests $ [] $ %{} 'TestEntry (:name |keeps-async-result-contract)
            :code $ quote $ assert= (%ok |ready) (%ok |ready)
            :tags $ #{} :wasm
        'host-combine $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-combine (left right count enabled?) |host |combine
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true) (:return 'String)
            :args $ [] 'String 'String 'Number 'Bool
        'host-flag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-flag (flag) |host |flag
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true) (:return 'Bool)
            :args $ [] 'Bool
        'host-http-request $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-http-request (request) |calcit:wasi-http/client |request
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'component-wasm-async-import.main/HttpRequest
            :return $ :: 'Result 'component-wasm-async-import.main/HttpResponse 'component-wasm-async-import.main/HttpError
        'host-load $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-load (text) |host |load
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'String
            :return $ :: 'Result 'String 'String
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns component-wasm-async-import.main
