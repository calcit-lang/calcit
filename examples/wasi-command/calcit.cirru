
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description "|最小 WASI command 文本处理示例") (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :wasm)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'Manifest $ %{} 'CodeEntry (:doc "|小型配置清单：具名、启用状态与版本号。")
          :code $ quote $ defstruct Manifest (:name 'String) (:enabled 'Bool) (:revision 'Int32)
          :examples $ []
          :schema $ :: 'StructDef
        'decode-manifest $ %{} 'CodeEntry (:doc "|把 Cirru EDN 文本明确解码为 Manifest；非法结构返回 Result 错误。")
          :code $ quote $ defn decode-manifest (text) (try-parse-cirru-edn-as text 'Manifest)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result 'app.main/Manifest 'String
        'fail! $ %{} 'CodeEntry (:doc "|向 stderr 输出错误并以指定状态码退出。")
          :code $ quote $ defn fail! (code message) (eprintln message) (quit! code)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'Number 'String
        'main! $ %{} 'CodeEntry (:doc "|读取 guest 文件，添加可选前缀并写入目标 guest 文件。")
          :code $ quote $ defn main! ()
            let
                args $ get-args
              if
                < (count args) 3
                fail! 64 "|Usage: <module> <input-guest-path> <output-guest-path>"
                let
                    input-path $ &list:nth args 1
                    output-path $ &list:nth args 2
                    prefix $ option:unwrap-or (get-env |WASI_PREFIX) |
                  match
                    .read-text $ fs:path input-path
                    (:ok content)
                      match
                        .write-text (fs:path output-path) (transform-content prefix content)
                        (:ok _) (println "|Wrote " output-path)
                        (:err message)
                          fail! 73 $ str "|Failed to write " output-path |: message
                    (:err message)
                      fail! 66 $ str "|Failed to read " input-path |: message
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'manifest-main! $ %{} 'CodeEntry (:doc "|从预开放目录读取、转换并写回配置清单；失败以稳定退出码报告。")
          :code $ quote $ defn manifest-main! ()
            match
              .read-text $ fs:path |workspace/input.cirru
              (:err message)
                fail! 66 $ str |input: message
              (:ok content)
                match (process-manifest |prod- content)
                  (:err message)
                    fail! 65 $ str |manifest: message
                  (:ok output)
                    match
                      .write-text (fs:path |workspace/output.cirru) output
                      (:err message)
                        fail! 73 $ str |output: message
                      (:ok _) (println |Manifest-written)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'method-eval-main! $ %{} 'CodeEntry
          :doc "|验收普通 Result.map 的 receiver 与回调实参副作用只各执行一次；不用于正式文件业务。"
          :code $ quote $ defn method-eval-main! ()
            let
                result $
                  do (println |receiver)
                    decode-manifest "|%{} 'Manifest (:name |api) (:enabled true) (:revision 3)"
                  , .map $ do (println |argument)
                    fn (manifest)
                      str (:name manifest) |!
              match result
                (:ok value) (println value)
                (:err message) (fail! 70 message)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'process-manifest $ %{} 'CodeEntry (:doc "|使用普通 Result 方法串联解码、业务变换和 Cirru EDN 输出。")
          :code $ quote $ defn process-manifest (prefix content)
            (decode-manifest content) .and-then $ fn (manifest)
              (transform-manifest prefix manifest) .map $ fn (updated) (format-cirru-edn updated)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String 'String
            :return $ :: 'Result 'String 'String
          :tests $ []
            %{} 'TestEntry (:name |transforms-typed-config)
              :code $ quote $ match
                process-manifest |prod- "|%{} 'Manifest (:name |api) (:enabled true) (:revision 3)"
                (:err message) (raise message)
                (:ok output)
                  match (decode-manifest output)
                    (:err message) (raise message)
                    (:ok value)
                      do
                        assert= |prod-api $ :name value
                        assert= true $ :enabled value
                        assert= 3 $ :revision value
              :tags $ #{} :unit :wasi
            %{} 'TestEntry (:name |rejects-invalid-config)
              :code $ quote $ do
                assert= true $ result:err? $ process-manifest |prod- "|%{} 'Manifest (:name |) (:enabled true) (:revision 3)"
                assert= true $ result:err? $ process-manifest |prod- "|%{} 'Manifest (:name |api) (:enabled true) (:revision -1)"
                assert= true $ result:err? $ process-manifest |prod- "|%{} 'Manifest (:name |api) (:enabled |yes) (:revision 3)"
              :tags $ #{} :unit :wasi
        'reload! $ %{} 'CodeEntry (:doc "|开发模式重载占位入口。")
          :code $ quote $ defn reload! () (println |Reloaded)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'transform-content $ %{} 'CodeEntry (:doc "|为文本内容添加由环境变量提供的前缀。")
          :code $ quote $ defn transform-content (prefix content) (str prefix content)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String 'String
          :tests $ [] $ %{} 'TestEntry (:name |prefixes-text)
            :code $ quote $ assert= "|prefix: payload" (transform-content "|prefix: " |payload)
            :tags $ #{} :unit :wasi
        'transform-manifest $ %{} 'CodeEntry (:doc "|校验清单并加上部署名称前缀，不改变启用状态或版本号。")
          :code $ quote $ defn transform-manifest (prefix manifest)
            let
                name $ :name manifest
                revision $ :revision manifest
              if
                or (= name |) (< revision 0)
                %err |invalid-manifest
                %ok $ Manifest :name (str prefix name) :enabled (:enabled manifest) :revision revision
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String 'app.main/Manifest
            :return $ :: 'Result 'app.main/Manifest 'String
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
