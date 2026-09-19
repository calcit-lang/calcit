
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |component-wasm-stream
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'component-wasm-stream.main/main!) (:mode :native) (:reload-fn 'component-wasm-stream.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'component-wasm-stream.main
    %{} 'FileEntry
      :defs $ {}
        'consume $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export consume (stream) (consume-readable-byte-stream stream 5 3 on-chunk)
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'ReadableByteStream
            :return $ :: 'Result 'Unit 'StreamConsumeError
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'on-chunk $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn on-chunk (chunk) true
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Buffer
          :tests $ [] $ %{} 'TestEntry (:name |accepts-buffer-chunks)
            :code $ quote $ assert= true
              on-chunk $ &buffer 1 2 3
            :tags $ #{} :wasm
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'stop $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export stop (stream) (consume-readable-byte-stream stream 5 3 stop-on-chunk)
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'ReadableByteStream
            :return $ :: 'Result 'Unit 'StreamConsumeError
        'stop-on-chunk $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn stop-on-chunk (chunk) false
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Buffer
          :tests $ [] $ %{} 'TestEntry (:name |requests-early-stop)
            :code $ quote $ assert= false
              stop-on-chunk $ &buffer 1
            :tags $ #{} :wasm
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns component-wasm-stream.main
