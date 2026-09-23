
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
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |returns-unit)
            :code $ quote $ assert= &unit (main!)
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
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'main-growth-loop! $ %{} 'CodeEntry (:doc "|验证大文件读取后反复构造小型 Result 值不会越界。")
          :code $ quote $ defn main-growth-loop! ()
            let
                indexes $ range 5000
                content $ result:unwrap-or (read-workspace-file |workspace/limit.txt) |missing
              assert= 4194304 $ count content
              each indexes $ fn (index)
                assert= true $ result:ok? $ %ok index
              println |Growth-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
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
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
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
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'main-unsupported! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main-unsupported! ()
            assert= true $ result:ok? $ .read-dir (fs:path |workspace)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'read-workspace-file $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-workspace-file (path)
            .read-text $ fs:path path
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result 'String 'String
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
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'transform-content $ %{} 'CodeEntry (:doc "|组合前缀和正文的纯文本转换。")
          :code $ quote $ defn transform-content (prefix content) (str prefix content)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String 'String
          :tests $ [] $ %{} 'TestEntry (:name |prefixes-text)
            :code $ quote $ assert= "|prefix: payload" (transform-content "|prefix: " |payload)
            :tags $ #{} :unit :wasi
        'write-workspace-file $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-workspace-file (path content)
            .write-text (fs:path path) content
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String 'String
            :return $ :: 'Result 'Unit 'String
          :tests $ [] $ %{} 'TestEntry (:name |denies-missing-root)
            :code $ quote $ assert= true
              result:err? $ write-workspace-file |/calcit-no-such-preopen/file.txt |content
            :tags $ #{} :unit :wasi
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
