
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
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
