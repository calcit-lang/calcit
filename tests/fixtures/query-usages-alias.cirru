{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description "|query usages resolves namespace aliases, refers and qualified names (#2236)") (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.schema $ %{} 'FileEntry
      :defs $ {}
        'decode-story $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn decode-story (x) x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'Story $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Story (:title 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'message-input $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn message-input (x) (decode-story x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
      :ns $ %{} 'CodeEntry (:doc |)
        :code $ quote $ ns app.schema
        :examples $ []
    'app.alias-caller $ %{} 'FileEntry
      :defs $ {}
        'reading-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reading-list (x) (schema/decode-story x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |alias-call-in-test)
            :code $ quote $ do
              assert= 1 $ schema/decode-story 1
              , &unit
            :tags $ #{} :query-usages
        'alias-story $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn alias-story (s) s
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'schema/Story)
            :args $ [] 'schema/Story
        'unrelated-bare $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unrelated-bare (decode-story) decode-story
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
      :ns $ %{} 'CodeEntry (:doc |)
        :code $ quote $ ns app.alias-caller $ :require (app.schema :as schema)
        :examples $ []
    'app.refer-caller $ %{} 'FileEntry
      :defs $ {}
        'refer-call $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn refer-call (x) (decode-story x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'refer-story $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn refer-story (s) s
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Story)
            :args $ [] 'Story
        'qualified-story $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn qualified-story (s) s
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.schema/Story)
            :args $ [] 'app.schema/Story
        'qualified-call $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn qualified-call (x) (app.schema/decode-story x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
      :ns $ %{} 'CodeEntry (:doc |)
        :code $ quote $ ns app.refer-caller $ :require (app.schema :refer $ decode-story Story)
        :examples $ []
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (println $ app.schema/decode-story 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'CodeEntry (:doc |)
        :code $ quote $ ns app.main
        :examples $ []
