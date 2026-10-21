
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |page-icons
  :entries $ {} $ :default
    {} (:description "|只由 page 入口加载的依赖模块 fixture") (:init-fn 'page-icons.core/main!) (:mode :native) (:reload-fn 'page-icons.core/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'page-icons.core
    %{} 'FileEntry
      :defs $ {}
        'comp-icon $ %{} 'CodeEntry (:doc "|按名称渲染图标标签。")
          :code $ quote $ defn comp-icon (icon-name) (&str:concat |icon: icon-name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns page-icons.core
