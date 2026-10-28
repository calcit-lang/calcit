
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
          :schema $ quote $ Fn Number (-> fix-command.reader/Plugin)
          :tests $ [] $ %{} 'TestEntry (:name |nominal-method)
            :code $ quote $ assert= 3
              .render $ %make-plugin 3
        'Plugin $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Plugin (impl-traits Plugin0 PluginImpl)
          :examples $ []
        'Plugin0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Plugin0 (:item 'Number)
          :examples $ []
          :schema $ quote EnumDef
        'PluginImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl PluginImpl PluginTrait (.render plugin-get)
          :examples $ []
          :schema $ quote Impl
        'PluginTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait PluginTrait (.render 'Fn)
          :examples $ []
          :schema $ quote Trait
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            assert= 3 $ .render $ %make-plugin 3
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'plugin-get $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plugin-get (self)
            match self
              (:item value) value
              _ $ raise |invalid-plugin
          :examples $ []
          :schema $ quote $ Fn fix-command.reader/Plugin (-> Number)
        'unused-enum-index $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unused-enum-index (key)
            .contains? (Option :some 1) key
          :examples $ []
          :schema $ quote $ Fn K (-> Bool) (:generics K)
          :tests $ []
            %{} 'TestEntry (:name |numeric-index)
              :code $ quote $ assert= true (unused-enum-index 1.5)
              :tags $ #{} :enum-numeric-evidence :unit
            %{} 'TestEntry (:name |rejects-nil-index)
              :code $ quote $ assert= "|Enum .contains? expected a Number index"
                try (unused-enum-index nil)
                  fn (message) message
              :tags $ #{} :enum-numeric-evidence :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.reader
