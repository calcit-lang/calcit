
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
        'main! $ %{} 'CodeEntry (:doc "|WASI 0.3 command Component 的零参数纯计算入口。")
          :code $ quote $ defn main! () (transform-content "|prefix: " |payload) &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |returns-unit)
            :code $ quote $ assert= &unit (main!)
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
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
