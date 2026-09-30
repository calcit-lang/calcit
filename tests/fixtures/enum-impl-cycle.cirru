
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |fix-command
  :entries $ {} $ :default
    {} (:description "|带 trait 的 Enum 构造与 self match 循环回归") (:init-fn 'fix-command.reader/main!) (:mode :native) (:reload-fn 'fix-command.reader/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'fix-command.reader
    %{} 'FileEntry
      :defs $ {}
        '%make-plugin $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn %make-plugin (value) (%:: Plugin :item value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'fix-command.reader/Plugin)
            :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nominal-method)
            :code $ quote $ assert= 3
              .render $ %make-plugin 3
        'Plugin $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Plugin (impl-traits Plugin0 PluginImpl)
          :examples $ []
        'Plugin0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Plugin0 (:item 'Number)
          :examples $ []
          :schema $ :: 'EnumDef
        'PluginImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl PluginImpl PluginTrait (.render plugin-get)
          :examples $ []
          :schema $ :: 'Impl
        'PluginTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait PluginTrait (.render :fn)
          :examples $ []
          :schema $ :: 'Trait
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            assert= 3 $ .render $ %make-plugin 3
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'plugin-get $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plugin-get (self)
            match self
              (:item value) value
              _ $ raise |invalid-plugin
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'fix-command.reader/Plugin
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.reader
