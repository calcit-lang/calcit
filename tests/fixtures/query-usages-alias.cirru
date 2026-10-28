
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {}
      :description "|query usages resolves namespace aliases, refers and qualified names (#2236)"
      :init-fn 'app.main/main!
      :mode :native
      :reload-fn 'app.main/main!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.alias-caller $ %{} 'FileEntry
      :defs $ {}
        'alias-story $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn alias-story (s) s
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'schema/Story)
            :args $ [] 'schema/Story
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
        'unrelated-bare $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unrelated-bare (decode-story) decode-story
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.alias-caller
          :require $ app.schema :as schema
    'app.core-fallback $ %{} 'FileEntry
      :defs $ {}
        'implicit $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn implicit (value) (identity value)
          :examples $ []
        'qualified $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn qualified (value) (calcit.core/identity value)
          :examples $ []
        'shadowed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn shadowed (identity) (identity true)
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.core-fallback
    'app.main $ %{} 'FileEntry
      :defs $ {} $ 'main!
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println $ app.schema/decode-story 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
    'app.refer-caller $ %{} 'FileEntry
      :defs $ {}
        'qualified-call $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn qualified-call (x) (app.schema/decode-story x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'qualified-story $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn qualified-story (s) s
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.schema/Story)
            :args $ [] 'app.schema/Story
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
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.refer-caller
          :require $ app.schema :refer $ decode-story Story
    'app.schema $ %{} 'FileEntry
      :defs $ {}
        'Story $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Story (:title 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'decode-story $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn decode-story (x) x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'message-input $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn message-input (x) (decode-story x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.schema
    'app.shadow $ %{} 'FileEntry
      :defs $ {}
        'alias-let-shadow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn alias-let-shadow ()
            core/let
                shadow-sentinel false
              , shadow-sentinel
          :examples $ []
        'compact-splice-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn compact-splice-read ()
            quasiquote $ [] ~@template-values
          :examples $ []
        'compact-unquote-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn compact-unquote-read ()
            quasiquote $ [] ~template-value
          :examples $ []
        'global-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn global-read () shadow-sentinel
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |scoped-test-data)
            :code $ quote $ let
                shadow-sentinel false
              assert= false shadow-sentinel
        'initializer-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn initializer-read ()
            let
                shadow-sentinel shadow-sentinel
              , shadow-sentinel
          :examples $ []
        'let-shadow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn let-shadow ()
            let
                shadow-sentinel false
              , shadow-sentinel
          :examples $ []
        'local-escape-shadow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn local-escape-shadow (template-value)
            quasiquote $ [] ~template-value $ ~ template-value
          :examples $ []
        'local-head-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn local-head-read (let)
            hint-fn $ quote $ Fn
              Fn Bool Bool $ -> Bool
              -> Bool
            let shadow-sentinel false
          :examples $ []
        'loop-shadow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn loop-shadow ()
            loop
                shadow-sentinel false
              , shadow-sentinel
          :examples $ []
        'nested-escape-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nested-escape-read ()
            quasiquote $ []
              quote $ [] ~template-value
              quasiquote $ [] $ ~ template-value
          :examples $ []
        'parameter-shadow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn parameter-shadow (shadow-sentinel) shadow-sentinel
          :examples $ []
        'qualified-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn qualified-read (shadow-sentinel) app.shadow/shadow-sentinel
          :examples $ []
        'quoted-data $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn quoted-data () (quote shadow-sentinel)
          :examples $ []
        'restored-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn restored-read ()
            fn (shadow-sentinel) shadow-sentinel
            , shadow-sentinel
          :examples $ []
        'sequential-shadow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn sequential-shadow ()
            let
                shadow-sentinel false
                later shadow-sentinel
              , later
          :examples $ []
        'shadow-sentinel $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def shadow-sentinel true
          :examples $ []
        'splice-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn splice-read ()
            quasiquote $ [] template-values $ ~@ template-values
          :examples $ []
        'template-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def template-value true
          :examples $ []
        'template-values $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def template-values ([] true)
          :examples $ []
        'unquote-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unquote-read ()
            quasiquote $ [] template-value $ ~ template-value
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.shadow
          :require $ calcit.core :as core
