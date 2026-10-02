
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |fix-command
  :entries $ {} $ :default
    {} (:description "|公开名称的依赖模块 fixture；仅通过 consumer 使用") (:init-fn 'fix-command.reader/main!) (:mode :native) (:reload-fn 'fix-command.reader/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'fix-command.alias-evidence $ %{} 'FileEntry
      :defs $ {}
        'AliasBase $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct AliasBase (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'AliasEnriched $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def AliasEnriched (impl-traits AliasBase AliasMarkerImpl)
          :examples $ []
          :schema $ :: 'StructDef
        'AliasMarker $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait AliasMarker
          :examples $ []
          :schema $ :: 'Trait
        'AliasMarkerImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl AliasMarkerImpl AliasMarker
          :examples $ []
          :schema $ :: 'Impl
        'ChoiceBase $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum ChoiceBase (:value 'Number)
          :examples $ []
          :schema $ :: 'EnumDef
        'ChoiceEnriched $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def ChoiceEnriched (impl-traits ChoiceBase AliasMarkerImpl)
          :examples $ []
          :schema $ :: 'EnumDef
        'OtherBase $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OtherBase (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'OtherChoice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum OtherChoice (:value 'Number)
          :examples $ []
          :schema $ :: 'EnumDef
        'apply-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def apply-alias fix-command.alias-evidence/apply-text
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |callback-context)
            :code $ quote $ do
              assert= |ok $ fix-command.alias-evidence/apply-alias fix-command.alias-evidence/echo-chain |ok
              assert= |hello! $ fix-command.alias-evidence/apply-alias
                fn (text) (&str:concat text |!)
                , |hello
            :tags $ #{} :alias-contract :unit
        'apply-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-text (f x)
            hint-fn $ {} (:return 'String)
              :args $ []
                :: 'Fn $ {} (:return 'String)
                  :args $ [] 'String
                , 'String
            f x
          :examples $ []
        'consume-base $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn consume-base (value) (:value value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'fix-command.alias-evidence/AliasBase
          :tests $ [] $ %{} 'TestEntry (:name |trait-bearing-producer)
            :code $ quote $ do
              assert= 1 $ fix-command.alias-evidence/consume-base $ fix-command.alias-evidence/produce-enriched
              assert= 2 $ fix-command.alias-evidence/consume-base $ fix-command.alias-evidence/AliasBase :value 2
              , &unit
            :tags $ #{} :alias-contract
        'consume-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn consume-choice (value) &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'fix-command.alias-evidence/ChoiceBase
          :tests $ [] $ %{} 'TestEntry (:name |trait-bearing-producer)
            :code $ quote $ do
              assert= &unit $ fix-command.alias-evidence/consume-choice $ fix-command.alias-evidence/produce-choice
              assert= &unit $ fix-command.alias-evidence/consume-choice $ fix-command.alias-evidence/ChoiceBase :value 2
              , &unit
            :tags $ #{} :alias-contract
        'echo-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def echo-alias fix-command.alias-evidence/echo-string
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
          :tests $ [] $ %{} 'TestEntry (:name |direct-and-local-values)
            :code $ quote $ let
                local-f fix-command.alias-evidence/echo-alias
              assert= |ok $ fix-command.alias-evidence/echo-alias |ok
              assert= |ok $ fix-command.alias-evidence/echo-chain |ok
              assert= |ok $ local-f |ok
              assert= ([] |a |b)
                map ([] |a |b) fix-command.alias-evidence/echo-chain
            :tags $ #{} :alias-contract :unit
        'echo-chain $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def echo-chain fix-command.alias-evidence/echo-alias
          :examples $ []
        'echo-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn echo-string (x) x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'empty-proc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def empty-proc &list:empty?
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |specializes-proc-alias-callback)
            :code $ quote $ do
              assert-type
                .map
                  [] ([]) ([] 1)
                  , fix-command.alias-evidence/empty-proc
                :: List Bool
              assert= ([] true false)
                .map
                  [] ([]) ([] 1)
                  , fix-command.alias-evidence/empty-proc
            :tags $ #{} :alias-contract :unit
        'fold-proc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def fold-proc &list:foldl
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |preserves-internal-proc)
            :code $ quote $ assert= 6
              fix-command.alias-evidence/fold-proc ([] 1 2 3) 0 &+
            :tags $ #{} :alias-contract :unit
        'list-question-proc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def list-question-proc &list?
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |preserves-internal-proc)
            :code $ quote $ do
              assert= true $ fix-command.alias-evidence/list-question-proc $ [] 1
              assert= false $ fix-command.alias-evidence/list-question-proc 1
            :tags $ #{} :alias-contract :unit
        'optional-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def optional-alias fix-command.alias-evidence/optional-text
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |optional-values)
            :code $ quote $ do
              assert= |fallback $ fix-command.alias-evidence/optional-alias
              assert= |ok $ fix-command.alias-evidence/optional-alias $ Option :some |ok
            :tags $ #{} :alias-contract :unit
        'optional-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn optional-text (x)
            hint-fn $ {} (:return 'String)
              :args $ [] $ :: 'Option 'String
            .unwrap-or x |fallback
          :examples $ []
        'produce-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn produce-choice () (ChoiceEnriched :value 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'fix-command.alias-evidence/ChoiceEnriched)
            :args $ []
        'produce-enriched $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn produce-enriched () (AliasEnriched :value 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'fix-command.alias-evidence/AliasEnriched)
            :args $ []
        'range-proc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def range-proc &list:range
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |preserves-internal-proc)
            :code $ quote $ assert= ([] 1 3 5) (fix-command.alias-evidence/range-proc 1 7 2)
            :tags $ #{} :alias-contract :unit
        'rest-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def rest-alias fix-command.alias-evidence/rest-count
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |rest-values)
            :code $ quote $ do
              assert= 0 $ fix-command.alias-evidence/rest-alias |first
              assert= 2 $ fix-command.alias-evidence/rest-alias |first |second |third
            :tags $ #{} :alias-contract :unit
        'rest-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn rest-count (x & more)
            hint-fn $ {} (:return 'Number) (:rest 'String)
              :args $ [] 'String
            &list:count more
          :examples $ []
        'same-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def same-alias fix-command.alias-evidence/same-first
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |generic-relations)
            :code $ quote $ do
              assert= 3 $ fix-command.alias-evidence/same-alias 3 4
              assert= |a $ fix-command.alias-evidence/same-alias |a |b
            :tags $ #{} :alias-contract :unit
        'same-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn same-first (left right)
            hint-fn $ {} (:return 'T)
              :generics $ [] 'T
              :args $ [] 'T 'T
            , left
          :examples $ []
        'shortcut-proc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def shortcut-proc &list:foldl-shortcut
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |preserves-internal-proc)
            :code $ quote $ assert= 3
              fix-command.alias-evidence/shortcut-proc ([] 1 2 3) 0 99 $ fn (acc x)
                :: (&= x 2) (&+ acc x)
            :tags $ #{} :alias-contract :unit
        'sort-proc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def sort-proc &list:sort
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |preserves-internal-proc)
            :code $ quote $ assert= ([] 1 2 3)
              fix-command.alias-evidence/sort-proc ([] 3 1 2) &-
            :tags $ #{} :alias-contract :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.alias-evidence
    'fix-command.left $ %{} 'FileEntry
      :defs $ {} $ 'state-alias
        %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias 20
          :examples $ []
          :schema $ :: 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.left
    'fix-command.other-side $ %{} 'FileEntry
      :defs $ {}
        'AliasBase $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct AliasBase (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'state-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias 40
          :examples $ []
          :schema $ :: 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.other-side
    'fix-command.other_side $ %{} 'FileEntry
      :defs $ {} $ 'state-alias
        %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias 50
          :examples $ []
          :schema $ :: 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.other_side
    'fix-command.reader $ %{} 'FileEntry
      :defs $ {}
        'dash-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def dash-value 60
          :examples $ []
          :schema $ :: 'Number
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'read-dir $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-dir (path) (&str:concat |reader:dir: path)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'read-file $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-file (path) (&str:concat |reader:file: path)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'state-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias 10
          :examples $ []
          :schema $ :: 'Number
          :tests $ [] $ %{} 'TestEntry (:name |namespace-identity)
            :code $ quote $ do (assert= 10 fix-command.reader/state-alias) (assert= 20 fix-command.left/state-alias) (assert= 30 fix-command.right/state-alias) (assert= 20 left/state-alias) (assert= 30 right/state-alias)
              let
                  state-alias 7
                assert= 7 state-alias
                assert= 10 fix-command.reader/state-alias
                assert= 20 fix-command.left/state-alias
                assert= 30 fix-command.right/state-alias
              assert= 40 fix-command.other-side/state-alias
              assert= 50 fix-command.other_side/state-alias
              let
                  $fix_command_DOT_left 99
                  $fix_command_DOT_left_ 98
                assert= 99 $fix_command_DOT_left
                assert= 98 $fix_command_DOT_left_
                assert= 20 fix-command.left/state-alias
                assert= 20 left/state-alias
              let
                  dash_value 61
                assert= 61 dash_value
                assert= 60 fix-command.reader/dash-value
            :tags $ #{} :alias-contract :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.reader
          :require (fix-command.left :as left) (fix-command.right :as right)
    'fix-command.right $ %{} 'FileEntry
      :defs $ {} $ 'state-alias
        %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias 30
          :examples $ []
          :schema $ :: 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.right
