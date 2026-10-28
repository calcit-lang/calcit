
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description "|WASI 0.3 command Component 参数列表回归 fixture") (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :wasm)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|通过退出码验证 WASI 0.3 命令参数的顺序与 UTF-8 内容。")
          :code $ quote $ defn main! ()
            if
              =
                &list:nth (get-args) 1
                , "|你好"
              quit! 7
              quit! 8
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
          :tests $ [] $ %{} 'TestEntry (:name |arguments-are-strings)
            :code $ quote $ assert= true
              every? (get-args) string?
            :tags $ #{} :unit :wasi
        'reload! $ %{} 'CodeEntry (:doc "|开发模式重载占位入口。")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
