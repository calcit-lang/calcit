
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description "|WASI 0.3 command Component 纯计算回归 fixture") (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :wasm)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|WASI 0.3 command Component 的标准输出与错误输出回归入口。")
          :code $ quote $ defn main! () (println "|你好" 42) (eprintln "|错误") (echo |done) &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
          :tests $ [] $ %{} 'TestEntry (:name |returns-unit)
            :code $ quote $ assert= &unit (main!)
        'main-capabilities! $ %{} 'CodeEntry (:doc "|验证 WASI 0.3 Component 时钟、等待与安全随机数能力。")
          :code $ quote $ defn main-capabilities! ()
            let
                wall $ unix-time-ms
                t0 $ monotonic-time-ms
                waited $ wait-ms 1
                t1 $ monotonic-time-ms
                rand0 $ secure-random-bytes 0
                rand16 $ secure-random-bytes 16
                rand-err $ secure-random-bytes -1
              assert= true $ > wall 0
              assert= true $ result:ok? waited
              assert= true $ >= t1 t0
              assert= true $ result:ok? rand0
              assert= true $ result:ok? rand16
              assert= true $ result:err? rand-err
              println |Capabilities-ok
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'main-dynamic-unsupported! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main-dynamic-unsupported! ()
            let
                action $ if
                  empty? $ get-args
                  , main-unsupported! reload!
              action
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'main-file! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main-file! ()
            assert= "|你好" $ result:unwrap-or (read-workspace-file |workspace/valid.txt) |missing
            assert= true $ result:err? $ read-workspace-file |workspace/invalid.txt
            assert= true $ result:err? $ read-workspace-file |workspace/oversized.txt
            assert= true $ result:err? $ read-workspace-file |workspace/no-such-file
            assert= true $ result:err? $ read-workspace-file |workspace/escape/secret.txt
            assert= true $ result:ok? $ write-workspace-file |workspace/written.txt "|你好"
            assert= "|你好" $ result:unwrap-or (read-workspace-file |workspace/written.txt) |missing
            assert= true $ result:err? $ write-workspace-file |workspace/escape/denied.txt |blocked
            assert= true $ result:err? $ write-workspace-file |workspace/no-dir/missing.txt |blocked
            assert= true $ result:ok? $ write-workspace-file |workspace/limit-output.txt
              result:unwrap-or (read-workspace-file |workspace/limit.txt) |missing
            assert=
              Result :ok $ [] (fs:path |workspace/listing/a.txt) (fs:path "|workspace/listing/子.txt")
              .read-dir $ fs:path |workspace/listing
            assert= true $ result:err? $ .read-dir (fs:path |workspace/no-such-dir)
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'main-growth-loop! $ %{} 'CodeEntry (:doc "|验证大文件读取后反复构造小型 Result 值不会越界。")
          :code $ quote $ defn main-growth-loop! ()
            let
                indexes $ range 5000
                content $ result:unwrap-or (read-workspace-file |workspace/limit.txt) |missing
              assert= 4194304 $ count content
              each indexes $ fn (index)
                assert= true $ result:ok? $ Result :ok index
              println |Growth-ok
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'main-overflow! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main-overflow! ()
            assert= 4194305 $ count $ str
              result:unwrap-or (read-workspace-file |workspace/limit.txt) |missing
              , |x
            assert= true $ result:err? $ write-workspace-file |workspace/oversized-output.txt
              str
                result:unwrap-or (read-workspace-file |workspace/limit.txt) |missing
                , |x
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'main-read-loop! $ %{} 'CodeEntry (:doc "|验证同一内联循环先读取成功文件、再读取缺失文件时稳定返回 Result 错误。")
          :code $ quote $ defn main-read-loop! ()
            each ([] |workspace/valid.txt |workspace/no-such-file |workspace/no-such-file |workspace/no-such-file)
              fn (path)
                let
                    result $ .read-text $ fs:path path
                  if (= path |workspace/valid.txt)
                    assert= true $ result:ok? result
                    assert= true $ result:err? result
            println |Read-loop-ok
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'main-unknown-alias! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main-unknown-alias! ()
            let
                action inc
              let
                  action $ if
                    empty? $ get-args
                    , inc dec
                println $ action 1
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'main-unsupported! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main-unsupported! ()
            assert= true $ result:ok? $ .walk-dir (fs:path |workspace)
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'main-unsupported-alias! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main-unsupported-alias! ()
            let
                action main-unsupported!
                invoke $ fn (callback)
                  hint-fn $ Fn
                    Fn (-> Unit) (:features :io)
                    -> Unit
                    :features :io
                  callback
              invoke action
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'read-workspace-file $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-workspace-file (path)
            .read-text $ fs:path path
          :examples $ []
          :schema $ quote $ Fn String
            -> $ Result String String
            :features :io
          :tests $ []
            %{} 'TestEntry (:name |reads-project-file)
              :code $ quote $ assert= true
                result:ok? $ read-workspace-file |Cargo.toml
              :tags $ #{} :unit :wasi
            %{} 'TestEntry (:name |reports-missing-file)
              :code $ quote $ assert= true
                result:err? $ read-workspace-file |workspace/no-such-file
              :tags $ #{} :unit :wasi
        'reload! $ %{} 'CodeEntry (:doc "|开发模式重载占位入口。")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'transform-content $ %{} 'CodeEntry (:doc "|组合前缀和正文的纯文本转换。")
          :code $ quote $ defn transform-content (prefix content) (str prefix content)
          :examples $ []
          :schema $ quote $ Fn String String (-> String)
          :tests $ [] $ %{} 'TestEntry (:name |prefixes-text)
            :code $ quote $ assert= "|prefix: payload" (transform-content "|prefix: " |payload)
            :tags $ #{} :unit :wasi
        'write-workspace-file $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-workspace-file (path content)
            .write-text! (fs:path path) content
          :examples $ []
          :schema $ quote $ Fn String String
            -> $ Result Unit String
            :features :io
          :tests $ [] $ %{} 'TestEntry (:name |denies-missing-root)
            :code $ quote $ assert= true
              result:err? $ write-workspace-file |/calcit-no-such-preopen/file.txt |content
            :tags $ #{} :unit :wasi
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
