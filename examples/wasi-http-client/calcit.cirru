
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |component-wasm-async-import
  :entries $ {} $ :default
    {}
      :description "|Practical buffered typed WASI HTTP client starter."
      :init-fn 'component-wasm-async-import.starter/main!
      :mode :native
      :reload-fn 'component-wasm-async-import.starter/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'component-wasm-async-import.starter
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
          :code $ quote $ defstruct HttpRequest (:method 'component-wasm-async-import.starter/HttpMethod) (:url 'String)
            :headers $ :: 'List 'component-wasm-async-import.starter/HttpHeader
            :body 'component-wasm-async-import.starter/HttpBody
            :max-response-bytes 'UInt64
          :examples $ []
          :schema $ :: 'StructDef
        'HttpResponse $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct HttpResponse (:status 'UInt16)
            :headers $ :: 'List 'component-wasm-async-import.starter/HttpHeader
            :body 'component-wasm-async-import.starter/HttpBody
          :examples $ []
          :schema $ :: 'StructDef
        'call-host-http-request $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-http-request (request) (host-http-request request)
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'component-wasm-async-import.starter/HttpRequest
            :return $ :: 'Result 'component-wasm-async-import.starter/HttpResponse 'component-wasm-async-import.starter/HttpError
          :tests $ []
            %{} 'TestEntry (:name |keeps-bounded-http-contract)
              :code $ quote $ match (number->uint64 1024)
                (:ok limit)
                  let
                      request $ HttpRequest :method (HttpMethod :get) :url |https://example.test/items :headers ([]) :body (HttpBody :empty) :max-response-bytes limit
                    assert= (HttpMethod :get) (:method request)
                    assert= |https://example.test/items $ :url request
                    assert= ([]) (:headers request)
                    assert= (HttpBody :empty) (:body request)
                    assert= limit $ :max-response-bytes request
                (:err message) (raise message)
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |keeps-http-success-shape)
              :code $ quote $ match (number->uint16 200)
                (:ok status)
                  let
                      response $ HttpResponse :status status :headers ([]) :body $ HttpBody :text |hello
                    assert= status $ :status response
                    assert= ([]) (:headers response)
                    assert= (HttpBody :text |hello) (:body response)
                (:err message) (raise message)
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |keeps-http-limit-error-shape)
              :code $ quote $ match (number->uint64 9)
                (:ok observed)
                  match
                    %err $ HttpError :response-too-large observed
                    (:err error)
                      match error
                        (:response-too-large actual) (assert= observed actual)
                        _ $ raise "|unexpected HTTP error"
                    (:ok _) (raise "|expected response-too-large")
                (:err message) (raise message)
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |keeps-http-capability-error-shape)
              :code $ quote $ match
                %err $ HttpError :capability-denied "|origin is not granted"
                (:err error)
                  match error
                    (:capability-denied message) (assert= "|origin is not granted" message)
                    _ $ raise "|unexpected HTTP error"
                (:ok _) (raise "|expected capability-denied")
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |keeps-http-invalid-request-shape)
              :code $ quote $ match
                %err $ HttpError :invalid-request "|invalid request URL"
                (:err error)
                  match error
                    (:invalid-request message) (assert= "|invalid request URL" message)
                    _ $ raise "|unexpected HTTP error"
                (:ok _) (raise "|expected invalid-request")
              :tags $ #{} :wasm
        'host-http-request $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-http-request (request) |calcit:wasi-http/client |request
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'component-wasm-async-import.starter/HttpRequest
            :return $ :: 'Result 'component-wasm-async-import.starter/HttpResponse 'component-wasm-async-import.starter/HttpError
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
        :code $ quote $ ns component-wasm-async-import.starter
