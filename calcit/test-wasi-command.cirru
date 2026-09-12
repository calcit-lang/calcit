
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |app)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :wasm)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'exit-7! $ %{} 'CodeEntry (:doc "|以状态码 7 终止进程，用于验证 command 退出边界。")
          :code $ quote
            defn exit-7! () $ quit! 7
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :control :wasi
        'exit-invalid! $ %{} 'CodeEntry (:doc "|以越界状态码终止进程，用于验证各 command backend 拒绝隐式取模。")
          :code $ quote
            defn exit-invalid! () $ quit! 256
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :control :wasi
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (println |WASI-stdout: "|你好") (echo |WASI-echo) (eprintln |WASI-stderr: 42)
              println |WASI-env: $ option:unwrap-or (get-env |CALCIT_WASI_TEST_ENV) |missing
              each (get-args)
                fn (arg) (println |WASI-arg: arg)
              + 1 2
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ []
          :tests $ []
            %{} 'TestEntry (:name |returns-three)
              :code $ quote
                assert= 3 $ main!
              :tags $ #{} :core :unit :wasi :wasm
        'needs-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn needs-arg (x) x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tests $ []
            %{} 'TestEntry (:name |returns-input)
              :code $ quote
                assert= 3 $ needs-arg 3
              :tags $ #{} :core :unit :wasi :wasm
        'read-args $ %{} 'CodeEntry (:doc "|读取当前宿主进程的完整参数列表。")
          :code $ quote
            defn read-args () $ get-args
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
              :features $ #{} :env :io
              :return $ :: 'List 'String
          :tests $ []
            %{} 'TestEntry (:name |returns-strings)
              :code $ quote
                assert= true $ every? (read-args) string?
              :tags $ #{} :core :env :unit :wasi :wasm
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns app.main $ :require
