
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |fix-command
  :entries $ {} $ :default
    {} (:description "|Compiler-guided source fix fixture.") (:init-fn 'fix-command.main/main!) (:mode :native) (:reload-fn 'fix-command.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'fix-command.main
    %{} 'FileEntry
      :defs $ {}
        'Label $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Label
            impl-traits
              defstruct Label $ :value 'String
              , LabelRenderImpl
          :examples $ []
          :schema $ :: 'StructDef
        'LabelRenderImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl LabelRenderImpl Renderable (.render render-label) (.identity label-identity)
          :examples $ []
          :schema $ :: 'Impl
        'Renderable $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait Renderable
            .render $ :: 'Fn $ {}
              :args $ [] 'T 'String
              :return 'String
              :generics $ [] 'T
            .identity $ :: 'Fn $ {}
              :args $ [] 'T 'U
              :return 'U
              :generics $ [] 'T 'U
          :examples $ []
          :schema $ :: 'Trait
        'checked-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-count (x) (.count x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'T
            :generics $ [] 'T
            :where $ {} $ 'T 'Countable
          :tests $ [] $ %{} 'TestEntry (:name |collection-method-results)
            :code $ quote $ do
              assert= 0 $ checked-count $ []
              assert= 3 $ checked-count $ [] 1 2 3
              assert= 3 $ checked-count |abc
              assert= 2 $ checked-count $ {} (:a 1) (:b 2)
              assert= 2 $ checked-count $ #{} :a :b
            :tags $ #{} :trait-return-proof :unit
        'identity-with-trait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn identity-with-trait (self value) (.identity self value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'U)
            :args $ [] 'T 'U
            :generics $ [] 'T 'U
            :where $ {} $ 'T 'Renderable
          :tests $ [] $ %{} 'TestEntry (:name |substitutes-generic-results)
            :code $ quote $ let
                label $ Label :value |Ada
              assert= 42 $ identity-with-trait label 42
              assert= |text $ identity-with-trait label |text
              assert= ([] 1 2)
                identity-with-trait label $ [] 1 2
            :tags $ #{} :trait-return-proof :unit
        'label-identity $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn label-identity (self value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'U)
            :args $ [] 'fix-command.main/Label 'U
            :generics $ [] 'U
        'main! $ %{} 'CodeEntry (:doc |Entry.)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'render-label $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn render-label (self prefix) prefix
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'fix-command.main/Label 'String
        'render-with-trait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn render-with-trait (value prefix) (.render value prefix)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'T 'String
            :generics $ [] 'T
            :where $ {} $ 'T 'Renderable
          :tests $ [] $ %{} 'TestEntry (:name |custom-trait-string-result)
            :code $ quote $ do
              assert= |prefix $ render-with-trait (Label :value |Ada) |prefix
              assert= | $ render-with-trait (Label :value |Bob) |
            :tags $ #{} :trait-return-proof :unit
        'typed-rest-forward $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn typed-rest-forward (xs)
            let
                sink $ fn (label & values)
                  hint-fn $ {}
                    :args $ [] 'String
                    :rest 'Number
                    :return 'Number
                  checked-count values
              sink |numbers & xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nonempty-and-empty)
            :code $ quote $ do
              assert= 3 $ typed-rest-forward $ [] 1 2 3
              assert= 0 $ typed-rest-forward $ []
            :tags $ #{} :unit
      :ns $ %{} 'NsEntry (:doc "|Fix command fixture.")
        :code $ quote $ ns fix-command.main
