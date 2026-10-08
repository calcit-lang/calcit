
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.binding-proof $ %{} 'FileEntry
      :defs $ {}
        'Box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Box
            :value $ :: 'Optional 'app.binding-proof/Value
          :examples $ []
          :schema $ :: 'StructDef
        'OpenBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OpenBox (:value 'Dynamic)
          :examples $ []
          :schema $ :: 'StructDef
        'OptionBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OptionBox
            :value $ :: 'Optional $ :: 'Option 'String
          :examples $ []
          :schema $ :: 'StructDef
        'Value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Value (:text 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'choose-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn choose-nil (present)
            let
                selected $ if present nil nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert= (Box :value nil) (choose-nil true)
              assert= (Box :value nil) (choose-nil false)
            :tags $ #{} :nullable-branch-binding
        'choose-nullable $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn choose-nullable (present box)
            let
                selected $ if present (:value box) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] 'Bool 'app.binding-proof/Box
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert= (Box :value nil)
                choose-nullable false $ Box :value $ Value :text |ok
              assert= (Box :value nil)
                choose-nullable true $ Box :value nil
              assert=
                Box :value $ Value :text |ok
                choose-nullable true $ Box :value $ Value :text |ok
            :tags $ #{} :nullable-branch-binding
        'choose-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn choose-option (present value)
            let
                selected $ if present value nil
              OptionBox :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/OptionBox)
            :args $ [] 'Bool $ :: 'Option 'String
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert= (OptionBox :value nil)
                choose-option false $ Option :some |ok
              assert=
                OptionBox :value $ Option :some |ok
                choose-option true $ Option :some |ok
              assert=
                OptionBox :value $ Option :none
                choose-option true $ Option :none
            :tags $ #{} :nullable-branch-binding
        'decode-lookup $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn decode-lookup (input)
            let
                selected $ match (input .get |key)
                  (:some raw)
                    match (try-decode-map-as raw app.binding-proof/Value)
                      (:ok typed) typed
                      (:err reason) nil
                  (:none) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Map 'String 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |checked-and-open-payloads)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                decode-lookup $ {} $ |key
                  {} $ :text |ok
              assert= (Box :value nil)
                decode-lookup $ {} $ |key 42
              assert= (Box :value nil)
                decode-lookup $ {} $ |other 42
            :tags $ #{} :open-match-payload
        'decode-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn decode-option (input)
            let
                selected $ match input
                  (:some raw)
                    match (try-decode-map-as raw app.binding-proof/Value)
                      (:ok typed) typed
                      (:err reason) nil
                  (:none) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Option 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |checked-and-open-payloads)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                decode-option $ Option :some $ {} (:text |ok)
              assert= (Box :value nil)
                decode-option $ Option :some 42
              assert= (Box :value nil)
                decode-option $ Option :none
            :tags $ #{} :open-match-payload
        'decode-result $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn decode-result (input)
            let
                selected $ match input
                  (:ok raw)
                    match (try-decode-map-as raw app.binding-proof/Value)
                      (:ok typed) typed
                      (:err reason) nil
                  (:err reason) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Result 'Dynamic 'String
          :tests $ [] $ %{} 'TestEntry (:name |checked-and-open-payloads)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                decode-result $ Result :ok $ {} (:text |ok)
              assert= (Box :value nil)
                decode-result $ Result :ok 42
              assert= (Box :value nil)
                decode-result $ Result :err |missing
            :tags $ #{} :open-match-payload
        'if-nil-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn if-nil-first (present)
            let
                selected $ if present nil $ Value :text |ok
                alias selected
              Box :value alias
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                if-nil-first false
              assert= (Box :value nil) (if-nil-first true)
            :tags $ #{} :nullable-branch-binding
        'if-value-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn if-value-first (present)
            let
                selected $ if present (Value :text |ok) nil
                alias selected
              Box :value alias
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                if-value-first true
              assert= (Box :value nil) (if-value-first false)
            :tags $ #{} :nullable-branch-binding
        'lookup-inline $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lookup-inline (items)
            Box :value $ match (items .get |key)
              (:some value) value
              (:none) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Map 'String 'app.binding-proof/Value
          :tests $ [] $ %{} 'TestEntry (:name |present-and-missing)
            :code $ quote $ do
              assert= (Box :value nil)
                lookup-inline $ {} $ |other (Value :text |ok)
              assert=
                Box :value $ Value :text |ok
                lookup-inline $ {} $ |key (Value :text |ok)
            :tags $ #{} :nullable-branch-binding
        'lookup-local $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lookup-local (items)
            let
                selected $ match (items .get |key)
                  (:some value) value
                  (:none) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Map 'String 'app.binding-proof/Value
          :tests $ [] $ %{} 'TestEntry (:name |present-and-missing)
            :code $ quote $ do
              assert= (Box :value nil)
                lookup-local $ {} $ |other (Value :text |ok)
              assert=
                Box :value $ Value :text |ok
                lookup-local $ {} $ |key (Value :text |ok)
            :tags $ #{} :nullable-branch-binding
        'retain-open $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn retain-open (input)
            let
                selected $ match input
                  (:some value) value
                  (:none) nil
              OpenBox :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/OpenBox)
            :args $ [] $ :: 'Option 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |checked-and-open-payloads)
            :code $ quote $ do
              assert= (Result :ok 42)
                try-decode-map-as
                  :value $ retain-open $ Option :some 42
                  , 'Number
              assert= (Result :ok |text)
                try-decode-map-as
                  :value $ retain-open $ Option :some |text
                  , 'String
              assert= true $ nil? $ :value
                retain-open $ Option :none
            :tags $ #{} :open-match-payload
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.binding-proof
    'app.dynamic-construction $ %{} 'FileEntry
      :defs $ {}
        'EmptyBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct EmptyBox
          :examples $ []
          :schema $ :: 'StructDef
        'PairBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct PairBox (:a 'Dynamic) (:b 'Dynamic)
          :examples $ []
          :schema $ :: 'StructDef
        'checked-create $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-create (prototype key value)
            try
              %{} prototype $ key value
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |scalar)
              :code $ quote $ do
                assert= (Box :value 1) (checked-create Box :value 1)
                assert= nil $ checked-create Box :value |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |list)
              :code $ quote $ do
                assert=
                  ListBox :items $ [] 1 2
                  checked-create ListBox :items $ [] 1 2
                assert= nil $ checked-create ListBox :items $ [] |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |map)
              :code $ quote $ do
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  checked-create MapBox :items $ {} $ :a ([] 1 nil)
                assert= nil $ checked-create MapBox :items $ {}
                  :a $ [] |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |set)
              :code $ quote $ do
                assert=
                  SetBox :items $ #{} 1 2
                  checked-create SetBox :items $ #{} 1 2
                assert= nil $ checked-create SetBox :items $ #{} |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |nominal)
              :code $ quote $ do
                assert=
                  NominalBox :user $ app.field-owner/User :name |one
                  checked-create NominalBox :user $ app.field-owner/User :name |one
                assert= nil $ checked-create NominalBox :user $ app.field-consumer/User :name |other
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |alias)
              :code $ quote $ do
                assert=
                  AliasBox :user $ app.field-owner/User :name |one
                  checked-create AliasBox :user $ app.field-owner/User :name |one
                assert= nil $ checked-create AliasBox :user $ app.field-consumer/User :name |other
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |applied)
              :code $ quote $ do
                assert=
                  AppliedBox :item $ GenericBox :items $ [] 1
                  checked-create AppliedBox :item $ GenericBox :items $ [] 1
                assert= nil $ checked-create AppliedBox :item $ GenericBox :items ([] |wrong)
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ do
                assert=
                  OptionBox :item $ Option :some 1
                  checked-create OptionBox :item $ Option :some 1
                assert= nil $ checked-create OptionBox :item $ Option :some |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |empty-and-open)
              :code $ quote $ do
                assert=
                  ListBox :items $ []
                  checked-create ListBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-create OpenBox :items $ [] 1 |two nil
                assert=
                  GenericBox :items $ [] 1
                  checked-create GenericBox :items $ [] 1
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |field-name-boundary)
              :code $ quote $ do
                assert=
                  ListBox :items $ [] 1
                  checked-create ListBox |items $ [] 1
                assert= nil $ checked-create ListBox :missing $ [] 1
                assert= nil $ checked-create ListBox 42 $ [] 1
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |symbol-field-name)
              :code $ quote $ assert=
                ListBox :items $ [] 1
                checked-create ListBox 'items $ [] 1
              :tags $ #{} :checked-struct-construction :unit
        'checked-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-empty (prototype)
            try (%{} prototype)
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef
          :tests $ [] $ %{} 'TestEntry (:name |zero-field-and-missing)
            :code $ quote $ do
              assert=
                checked-map EmptyBox $ {}
                checked-empty EmptyBox
              assert= true $ struct? $ checked-empty EmptyBox
              assert= nil $ checked-empty PairBox
              assert= nil $ checked-map EmptyBox $ {} (:extra nil)
            :tags $ #{} :checked-struct-construction :unit
        'checked-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-map (prototype fields)
            try (&struct:from-map prototype fields)
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef $ :: 'Map 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |complete-and-immutable)
              :code $ quote $ let
                  fields $ {} (:b |two) (:a nil)
                assert= (PairBox :a nil :b |two) (checked-map PairBox fields)
                assert=
                  {} (:b |two) (:a nil)
                  , fields
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |missing-and-extra)
              :code $ quote $ do
                assert= nil $ checked-map PairBox $ {} (:a nil)
                assert= nil $ checked-map PairBox $ {} (:a nil) (:b nil) (:extra nil)
                assert= nil $ checked-map PairBox $ {} (:a nil) (:extra nil)
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |normalized-duplicate)
              :code $ quote $ do
                assert= nil $ checked-map PairBox $ {} (:a nil) (|a 2)
                assert= nil $ checked-map PairBox $ {} (|a nil) (|b 2) (:a 3)
              :tags $ #{} :checked-struct-construction :unit
        'checked-map-field $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-map-field (prototype key value)
            try
              &struct:from-map prototype $ {} $ key value
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |scalar)
              :code $ quote $ do
                assert= (Box :value 1) (checked-map-field Box :value 1)
                assert= nil $ checked-map-field Box :value |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |list)
              :code $ quote $ do
                assert=
                  ListBox :items $ [] 1 2
                  checked-map-field ListBox :items $ [] 1 2
                assert= nil $ checked-map-field ListBox :items $ [] |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |map)
              :code $ quote $ do
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  checked-map-field MapBox :items $ {} $ :a ([] 1 nil)
                assert= nil $ checked-map-field MapBox :items $ {}
                  :a $ [] |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |set)
              :code $ quote $ do
                assert=
                  SetBox :items $ #{} 1 2
                  checked-map-field SetBox :items $ #{} 1 2
                assert= nil $ checked-map-field SetBox :items $ #{} |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |nominal)
              :code $ quote $ do
                assert=
                  NominalBox :user $ app.field-owner/User :name |one
                  checked-map-field NominalBox :user $ app.field-owner/User :name |one
                assert= nil $ checked-map-field NominalBox :user $ app.field-consumer/User :name |other
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |alias)
              :code $ quote $ do
                assert=
                  AliasBox :user $ app.field-owner/User :name |one
                  checked-map-field AliasBox :user $ app.field-owner/User :name |one
                assert= nil $ checked-map-field AliasBox :user $ app.field-consumer/User :name |other
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |applied)
              :code $ quote $ do
                assert=
                  AppliedBox :item $ GenericBox :items $ [] 1
                  checked-map-field AppliedBox :item $ GenericBox :items $ [] 1
                assert= nil $ checked-map-field AppliedBox :item $ GenericBox :items ([] |wrong)
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ do
                assert=
                  OptionBox :item $ Option :some 1
                  checked-map-field OptionBox :item $ Option :some 1
                assert= nil $ checked-map-field OptionBox :item $ Option :some |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |empty-and-open)
              :code $ quote $ do
                assert=
                  ListBox :items $ []
                  checked-map-field ListBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-map-field OpenBox :items $ [] 1 |two nil
                assert=
                  GenericBox :items $ [] 1
                  checked-map-field GenericBox :items $ [] 1
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |field-name-boundary)
              :code $ quote $ do
                assert=
                  ListBox :items $ [] 1
                  checked-map-field ListBox |items $ [] 1
                assert= nil $ checked-map-field ListBox :missing $ [] 1
                assert= nil $ checked-map-field ListBox 42 $ [] 1
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |symbol-map-key-rejected)
              :code $ quote $ assert= nil
                checked-map-field ListBox 'items $ [] 1
              :tags $ #{} :checked-struct-construction :unit
        'checked-pairs $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-pairs (prototype k1 v1 k2 v2)
            try
              %{} prototype (k1 v1) (k2 v2)
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef 'Dynamic 'Dynamic 'Dynamic 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |complete-order-and-duplicate)
            :code $ quote $ do
              assert= (PairBox :a nil :b |two) (checked-pairs PairBox :b |two :a nil)
              assert= nil $ checked-pairs PairBox :a nil |a 2
              assert= nil $ checked-pairs PairBox :missing nil :b 2
            :tags $ #{} :checked-struct-construction :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.dynamic-construction
          :require $ app.dynamic-write :refer $ Box ListBox MapBox SetBox NominalBox AliasBox AppliedBox GenericBox OptionBox OpenBox
    'app.dynamic-write $ %{} 'FileEntry
      :defs $ {}
        'AliasBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct AliasBox (:user 'app.dynamic-write/UserAlias)
          :examples $ []
          :schema $ :: 'StructDef
        'AppliedBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct AppliedBox
            :item $ :: 'app.dynamic-write/GenericBox 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'Box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Box (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'GenericBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct GenericBox ([] 'T)
            :items $ :: 'List 'T
          :examples $ []
          :schema $ :: 'StructDef
        'ListBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ListBox
            :items $ :: 'List 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'MapBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct MapBox
            :items $ :: 'Map 'Tag $ :: 'List (:: 'Optional 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'NominalBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NominalBox (:user 'app.field-owner/User)
          :examples $ []
          :schema $ :: 'StructDef
        'OpenBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OpenBox
            :items $ :: 'List 'Dynamic
          :examples $ []
          :schema $ :: 'StructDef
        'OptionBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OptionBox
            :item $ :: 'Option 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'SetBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct SetBox
            :items $ :: 'Set 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'UserAlias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def UserAlias app.field-owner/User
          :examples $ []
          :schema $ :: 'StructDef
        'checked-assoc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-assoc (base key value)
            if (struct? base)
              try (&struct:assoc base key value)
                fn (_error) nil
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |runtime-field-check)
              :code $ quote $ let
                  base $ Box :value 1
                assert= (Box :value 2) (checked-assoc base :value 2)
                assert= nil $ checked-assoc base :value |wrong
                assert= nil $ checked-assoc base :missing 2
                assert= (Box :value 1) base
              :tags $ #{} :checked-struct-write :struct-boundary :unit
            %{} 'TestEntry (:name |reject-wrong-list-element)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert= nil $ checked-assoc base :items $ [] |wrong
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :deep-struct-boundary :unit
            %{} 'TestEntry (:name |reject-foreign-nominal)
              :code $ quote $ let
                  user $ app.field-owner/User :name |original
                  foreign $ app.field-consumer/User :name |foreign
                  base $ NominalBox :user user
                assert= nil $ checked-assoc base :user foreign
                assert= (NominalBox :user user) base
              :tags $ #{} :checked-struct-write :nominal-struct-boundary :unit
            %{} 'TestEntry (:name |valid-list)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert=
                  ListBox :items $ [] 2 3
                  checked-assoc base |items $ [] 2 3
                assert=
                  ListBox :items $ []
                  checked-assoc base :items $ []
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nested-map)
              :code $ quote $ let
                  base $ MapBox :items $ {}
                    :a $ [] 1 nil
                assert=
                  MapBox :items $ {} $ :b ([] 2 nil)
                  checked-assoc base :items $ {} $ :b ([] 2 nil)
                assert=
                  MapBox :items $ {}
                  checked-assoc base :items $ {}
                assert= nil $ checked-assoc base :items $ {}
                  |a $ [] 1
                assert= nil $ checked-assoc base :items $ {}
                  :a $ [] |wrong
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |set-members)
              :code $ quote $ let
                  base $ SetBox :items $ #{} 1
                assert=
                  SetBox :items $ #{} 2 3
                  checked-assoc base :items $ #{} 2 3
                assert= nil $ checked-assoc base :items $ #{} |wrong
                assert=
                  SetBox :items $ #{} 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nominal-alias)
              :code $ quote $ let
                  user $ app.field-owner/User :name |one
                  next $ app.field-owner/User :name |two
                  base $ AliasBox :user user
                assert= (AliasBox :user next) (checked-assoc base :user next)
                assert= nil $ checked-assoc base :user $ app.field-consumer/User :name |wrong
                assert= (AliasBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |applied-nominal)
              :code $ quote $ let
                  one $ GenericBox :items $ [] 1
                  two $ GenericBox :items $ [] 2
                  base $ AppliedBox :item one
                assert= (AppliedBox :item two) (checked-assoc base :item two)
                assert= nil $ checked-assoc base :item $ GenericBox :items ([] |wrong)
                assert= (AppliedBox :item one) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ let
                  base $ OptionBox :item $ Option :none
                assert=
                  OptionBox :item $ Option :some 2
                  checked-assoc base :item $ Option :some 2
                assert= nil $ checked-assoc base :item $ Option :some |wrong
                assert=
                  OptionBox :item $ Option :none
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-leaf)
              :code $ quote $ let
                  base $ OpenBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-assoc base :items $ [] 1 |two nil
              :tags $ #{} :checked-struct-write :unit
        'checked-assoc-at $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-assoc-at (base key value)
            if (struct? base)
              try (&struct:assoc-at base 0 key value)
                fn (_error) nil
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |runtime-field-check)
              :code $ quote $ let
                  base $ Box :value 1
                assert= (Box :value 2) (checked-assoc-at base :value 2)
                assert= nil $ checked-assoc-at base :value |wrong
                assert= nil $ checked-assoc-at base :missing 2
                assert= (Box :value 1) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |reject-wrong-list-element)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert= nil $ checked-assoc-at base :items $ [] |wrong
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |reject-foreign-nominal)
              :code $ quote $ let
                  user $ app.field-owner/User :name |original
                  foreign $ app.field-consumer/User :name |foreign
                  base $ NominalBox :user user
                assert= nil $ checked-assoc-at base :user foreign
                assert= (NominalBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |valid-list)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert=
                  ListBox :items $ [] 2 3
                  checked-assoc-at base :items $ [] 2 3
                assert=
                  ListBox :items $ []
                  checked-assoc-at base :items $ []
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nested-map)
              :code $ quote $ let
                  base $ MapBox :items $ {}
                    :a $ [] 1 nil
                assert=
                  MapBox :items $ {} $ :b ([] 2 nil)
                  checked-assoc-at base :items $ {} $ :b ([] 2 nil)
                assert=
                  MapBox :items $ {}
                  checked-assoc-at base :items $ {}
                assert= nil $ checked-assoc-at base :items $ {}
                  |a $ [] 1
                assert= nil $ checked-assoc-at base :items $ {}
                  :a $ [] |wrong
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |set-members)
              :code $ quote $ let
                  base $ SetBox :items $ #{} 1
                assert=
                  SetBox :items $ #{} 2 3
                  checked-assoc-at base :items $ #{} 2 3
                assert= nil $ checked-assoc-at base :items $ #{} |wrong
                assert=
                  SetBox :items $ #{} 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nominal-alias)
              :code $ quote $ let
                  user $ app.field-owner/User :name |one
                  next $ app.field-owner/User :name |two
                  base $ AliasBox :user user
                assert= (AliasBox :user next) (checked-assoc-at base :user next)
                assert= nil $ checked-assoc-at base :user $ app.field-consumer/User :name |wrong
                assert= (AliasBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |applied-nominal)
              :code $ quote $ let
                  one $ GenericBox :items $ [] 1
                  two $ GenericBox :items $ [] 2
                  base $ AppliedBox :item one
                assert= (AppliedBox :item two) (checked-assoc-at base :item two)
                assert= nil $ checked-assoc-at base :item $ GenericBox :items ([] |wrong)
                assert= (AppliedBox :item one) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ let
                  base $ OptionBox :item $ Option :none
                assert=
                  OptionBox :item $ Option :some 2
                  checked-assoc-at base :item $ Option :some 2
                assert= nil $ checked-assoc-at base :item $ Option :some |wrong
                assert=
                  OptionBox :item $ Option :none
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-leaf)
              :code $ quote $ let
                  base $ OpenBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-assoc-at base :items $ [] 1 |two nil
              :tags $ #{} :checked-struct-write :unit
        'checked-with $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-with (base key value)
            if (struct? base)
              try (&struct:with base key value)
                fn (_error) nil
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |runtime-field-check)
              :code $ quote $ let
                  base $ Box :value 1
                assert= (Box :value 2) (checked-with base :value 2)
                assert= nil $ checked-with base :value |wrong
                assert= nil $ checked-with base :missing 2
                assert= (Box :value 1) base
              :tags $ #{} :checked-struct-write :struct-boundary :unit
            %{} 'TestEntry (:name |reject-wrong-list-element)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert= nil $ checked-with base :items $ [] |wrong
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :deep-struct-boundary :unit
            %{} 'TestEntry (:name |reject-foreign-nominal)
              :code $ quote $ let
                  user $ app.field-owner/User :name |original
                  foreign $ app.field-consumer/User :name |foreign
                  base $ NominalBox :user user
                assert= nil $ checked-with base :user foreign
                assert= (NominalBox :user user) base
              :tags $ #{} :checked-struct-write :nominal-struct-boundary :unit
            %{} 'TestEntry (:name |valid-list)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert=
                  ListBox :items $ [] 2 3
                  checked-with base |items $ [] 2 3
                assert=
                  ListBox :items $ []
                  checked-with base :items $ []
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nested-map)
              :code $ quote $ let
                  base $ MapBox :items $ {}
                    :a $ [] 1 nil
                assert=
                  MapBox :items $ {} $ :b ([] 2 nil)
                  checked-with base :items $ {} $ :b ([] 2 nil)
                assert=
                  MapBox :items $ {}
                  checked-with base :items $ {}
                assert= nil $ checked-with base :items $ {}
                  |a $ [] 1
                assert= nil $ checked-with base :items $ {}
                  :a $ [] |wrong
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |set-members)
              :code $ quote $ let
                  base $ SetBox :items $ #{} 1
                assert=
                  SetBox :items $ #{} 2 3
                  checked-with base :items $ #{} 2 3
                assert= nil $ checked-with base :items $ #{} |wrong
                assert=
                  SetBox :items $ #{} 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nominal-alias)
              :code $ quote $ let
                  user $ app.field-owner/User :name |one
                  next $ app.field-owner/User :name |two
                  base $ AliasBox :user user
                assert= (AliasBox :user next) (checked-with base :user next)
                assert= nil $ checked-with base :user $ app.field-consumer/User :name |wrong
                assert= (AliasBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |applied-nominal)
              :code $ quote $ let
                  one $ GenericBox :items $ [] 1
                  two $ GenericBox :items $ [] 2
                  base $ AppliedBox :item one
                assert= (AppliedBox :item two) (checked-with base :item two)
                assert= nil $ checked-with base :item $ GenericBox :items ([] |wrong)
                assert= (AppliedBox :item one) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ let
                  base $ OptionBox :item $ Option :none
                assert=
                  OptionBox :item $ Option :some 2
                  checked-with base :item $ Option :some 2
                assert= nil $ checked-with base :item $ Option :some |wrong
                assert=
                  OptionBox :item $ Option :none
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-leaf)
              :code $ quote $ let
                  base $ OpenBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-with base :items $ [] 1 |two nil
              :tags $ #{} :checked-struct-write :unit
        'checked-with-at $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-with-at (base key value)
            if (struct? base)
              try (&struct:with-at base 0 key value)
                fn (_error) nil
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |runtime-field-check)
              :code $ quote $ let
                  base $ Box :value 1
                assert= (Box :value 2) (checked-with-at base :value 2)
                assert= nil $ checked-with-at base :value |wrong
                assert= nil $ checked-with-at base :missing 2
                assert= (Box :value 1) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |reject-wrong-list-element)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert= nil $ checked-with-at base :items $ [] |wrong
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |reject-foreign-nominal)
              :code $ quote $ let
                  user $ app.field-owner/User :name |original
                  foreign $ app.field-consumer/User :name |foreign
                  base $ NominalBox :user user
                assert= nil $ checked-with-at base :user foreign
                assert= (NominalBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |valid-list)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert=
                  ListBox :items $ [] 2 3
                  checked-with-at base :items $ [] 2 3
                assert=
                  ListBox :items $ []
                  checked-with-at base :items $ []
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nested-map)
              :code $ quote $ let
                  base $ MapBox :items $ {}
                    :a $ [] 1 nil
                assert=
                  MapBox :items $ {} $ :b ([] 2 nil)
                  checked-with-at base :items $ {} $ :b ([] 2 nil)
                assert=
                  MapBox :items $ {}
                  checked-with-at base :items $ {}
                assert= nil $ checked-with-at base :items $ {}
                  |a $ [] 1
                assert= nil $ checked-with-at base :items $ {}
                  :a $ [] |wrong
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |set-members)
              :code $ quote $ let
                  base $ SetBox :items $ #{} 1
                assert=
                  SetBox :items $ #{} 2 3
                  checked-with-at base :items $ #{} 2 3
                assert= nil $ checked-with-at base :items $ #{} |wrong
                assert=
                  SetBox :items $ #{} 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nominal-alias)
              :code $ quote $ let
                  user $ app.field-owner/User :name |one
                  next $ app.field-owner/User :name |two
                  base $ AliasBox :user user
                assert= (AliasBox :user next) (checked-with-at base :user next)
                assert= nil $ checked-with-at base :user $ app.field-consumer/User :name |wrong
                assert= (AliasBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |applied-nominal)
              :code $ quote $ let
                  one $ GenericBox :items $ [] 1
                  two $ GenericBox :items $ [] 2
                  base $ AppliedBox :item one
                assert= (AppliedBox :item two) (checked-with-at base :item two)
                assert= nil $ checked-with-at base :item $ GenericBox :items ([] |wrong)
                assert= (AppliedBox :item one) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ let
                  base $ OptionBox :item $ Option :none
                assert=
                  OptionBox :item $ Option :some 2
                  checked-with-at base :item $ Option :some 2
                assert= nil $ checked-with-at base :item $ Option :some |wrong
                assert=
                  OptionBox :item $ Option :none
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-leaf)
              :code $ quote $ let
                  base $ OpenBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-with-at base :items $ [] 1 |two nil
              :tags $ #{} :checked-struct-write :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.dynamic-write
    'app.empty-fields $ %{} 'FileEntry
      :defs $ {}
        'Fields $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Fields
            :mapping $ :: 'Optional $ :: 'Map 'Tag 'String
            :items $ :: 'Optional $ :: 'List 'Number
            :members $ :: 'Optional $ :: 'Set 'String
          :examples $ []
          :schema $ :: 'StructDef
        'open-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn open-map () ({})
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Map 'Tag 'Dynamic
        'verify-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-empty ()
            let
                value $ Fields :mapping ({}) :items ([]) :members $ #{}
              assert= ({}) (:mapping value)
              assert= ([]) (:items value)
              assert= (#{}) (:members value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |optional-empty-container-literals)
            :code $ quote $ verify-empty
            :tags $ #{} :optional-empty-field
        'verify-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-nil ()
            let
                value $ Fields :mapping nil :items nil :members nil
              assert= nil $ :mapping value
              assert= nil $ :items value
              assert= nil $ :members value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |optional-nil-container-fields)
            :code $ quote $ verify-nil
            :tags $ #{} :optional-empty-field
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.empty-fields
    'app.field-consumer $ %{} 'FileEntry
      :defs $ {}
        'User $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct User (:name 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'make-local-user $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-local-user () (User :name |Caller)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.field-consumer/User)
            :args $ []
        'verify-generic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-generic ()
            let
                user $ owner/make-user
                value $ owner/Envelope :user user :value 42
              assert-type (:value value) 'Number
              assert= 42 $ :value value
              assert= |Ada $ :name $ :user value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |declaration-owned-generic)
            :code $ quote $ verify-generic
            :tags $ #{} :struct-field-origin
        'verify-generic-origin $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-generic-origin ()
            let
                value $ owner/Envelope :user (owner/make-user) :value $ make-local-user
              assert-type (:value value) 'app.field-consumer/User
              assert= |Caller $ :name $ :value value
              assert= |Ada $ :name $ :user value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |caller-owned-generic-argument)
            :code $ quote $ verify-generic-origin
            :tags $ #{} :struct-field-origin
        'verify-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-map ()
            let
                user $ owner/make-user
                db $ owner/Database :users
                  {} $ |one user
                  , :maybe nil
                found $ -> (:users db) (.get |one) (.unwrap)
              assert= |Ada $ :name found
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |declaration-owned-map)
            :code $ quote $ verify-map
            :tags $ #{} :struct-field-origin
        'verify-optional $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-optional ()
            let
                user $ owner/make-user
                db $ owner/Database :users
                  {} $ |one user
                  , :maybe user
              assert= user $ :maybe db
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |declaration-owned-optional)
            :code $ quote $ verify-optional
            :tags $ #{} :struct-field-origin
        'verify-update $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-update ()
            let
                user $ owner/make-user
                original $ owner/Database :users
                  {} $ |one user
                  , :maybe nil
                updated $ original .assoc :maybe user
                changed $ updated .assoc :users $ {} (|two user)
              assert= nil $ :maybe original
              assert= user $ :maybe changed
              assert= (Option :none)
                get (:users original) |two
              assert= (Option :some user)
                get (:users changed) |two
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |declaration-owned-update)
            :code $ quote $ verify-update
            :tags $ #{} :struct-field-origin
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.field-consumer
          :require $ app.field-owner :as owner
    'app.field-owner $ %{} 'FileEntry
      :defs $ {}
        'Database $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Database
            :users $ :: 'Map 'String 'User
            :maybe $ :: 'Optional 'User
          :examples $ []
          :schema $ :: 'StructDef
        'Envelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Envelope ([] 'T) (:user 'User) (:value 'T)
          :examples $ []
          :schema $ :: 'StructDef
        'User $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct User (:name 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'make-user $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-user () (User :name |Ada)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.field-owner/User)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.field-owner
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'answer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def answer 42
          :examples $ []
          :schema $ :: 'Number
        'initial-state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def initial-state values/initial-state
          :examples $ []
          :schema $ :: 'app.values/LoginState
        'label $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def label |Ada
          :examples $ []
          :schema $ :: 'String
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (verify-values) &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'members $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def members
            {} $ 1 |Ada
          :examples $ []
          :schema $ :: 'Map 'Number 'String
        'open-store $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def open-store members
          :examples $ []
          :schema $ :: 'Dynamic
        'prepare-client-patch $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn prepare-client-patch (next-raw)
            match (try-decode-map-as next-raw app.values/ClientProjection)
              (:ok typed)
                Result :ok $ app.values/PatchCandidate :raw next-raw :typed typed
              (:err reason) (Result :err reason)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :return $ :: 'Result 'app.values/PatchCandidate 'String
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'state-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias initial-state
          :examples $ []
          :schema $ :: 'app.values/LoginState
        'verify-login-decode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-login-decode ()
            match
              try-decode-map-as
                {} (:username |Ada) (:password |secret)
                , app.values/LoginState
              (:ok state)
                assert= |Ada $ :username state
              (:err reason) (raise reason)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |checked-login-projection)
              :code $ quote $ do (verify-login-decode)
                match
                  try-decode-map-as
                    {} (:username |Ada) (:password 42)
                    , app.values/LoginState
                  (:ok _) (raise |invalid-password-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |password
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |persisted-nested-value)
              :code $ quote $ do
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ {} $ :years ([] 2025 2026)
                    :: 'Map 'Tag $ :: 'List 'Number
                  (:ok data)
                    assert=
                      Option :some $ [] 2025 2026
                      get data :years
                  (:err reason) (raise reason)
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ {} $ :years ([] 2025 |bad)
                    :: 'Map 'Tag $ :: 'List 'Number
                  (:ok _) (raise |invalid-storage-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |[1]
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |member-key-contract)
              :code $ quote $ do
                match
                  try-decode-map-as members $ :: 'Map 'Number 'String
                  (:ok data)
                    assert= (Option :some |Ada) (get data 1)
                  (:err reason) (raise reason)
                match
                  try-decode-map-as members $ :: 'Map 'String 'String
                  (:ok _) (raise |numeric-member-key-accepted-as-string)
                  (:err reason)
                    assert= true $ .includes? reason |key
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |restored-credentials)
              :code $ quote $ do
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ [] |Ada |secret
                    :: 'List 'String
                  (:ok credentials)
                    assert= ([] |Ada |secret) credentials
                  (:err reason) (raise reason)
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ [] |Ada 42
                    :: 'List 'String
                  (:ok _) (raise |invalid-credentials-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |[1]
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |nominal-component-state)
              :code $ quote $ do (verify-values)
                assert= |Ada $ :username $ assoc initial-state :username |Ada
                assert= | $ :username initial-state
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |checked-patch-projection)
              :code $ quote $ let
                  previous-raw $ {} (:count 1) (:color |red)
                  previous $ match (prepare-client-patch previous-raw)
                    (:ok candidate) candidate
                    (:err reason) (raise reason)
                  next-raw $ assoc previous-raw :count 2
                match (prepare-client-patch next-raw)
                  (:ok candidate)
                    do
                      assert= next-raw $ :raw candidate
                      assert= 2 $ :count $ :typed candidate
                      assert= |red $ :color $ :typed candidate
                      assert= previous-raw $ :raw previous
                      assert= 1 $ :count $ :typed previous
                  (:err reason) (raise reason)
                match
                  prepare-client-patch $ assoc next-raw :count |invalid
                  (:ok _) (raise |invalid-patch-candidate-accepted)
                  (:err reason)
                    do
                      assert= true $ .includes? reason |count
                      assert= previous-raw $ :raw previous
                      assert= 1 $ :count $ :typed previous
                match
                  prepare-client-patch $ {} $ :count 2
                  (:ok _) (raise |incomplete-patch-candidate-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |color
              :tags $ #{} :diary-boundary :unit
        'verify-values $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-values ()
            assert= | $ :username initial-state
            assert= | $ :username state-alias
            assert= | $ :password reader/state-alias
            assert= 1 $ .len members
            assert= 42 answer
            assert= |Ada label
            assert= members open-store
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |retains-typed-top-level-values)
            :code $ quote $ verify-values
            :tags $ #{} :def-value-contract :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
          :require (app.reader :as reader) (app.values :as values)
    'app.reader $ %{} 'FileEntry
      :defs $ {} $ 'state-alias
        %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias values/initial-state
          :examples $ []
          :schema $ :: 'app.values/LoginState
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.reader
          :require $ app.values :as values
    'app.short-circuit $ %{} 'FileEntry
      :defs $ {}
        'common-or $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn common-or (k flag)
            if
              or
                and (number? k) flag
                and (number? k) (&>= k 0)
              &+ k 1
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |both-or-paths)
            :code $ quote $ do
              assert= 2 $ common-or 1 false
              assert= -1 $ common-or -2 true
              assert= 0 $ common-or |bad true
            :tags $ #{} :short-circuit-proof :unit
        'compound $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn compound (k)
            if
              and (number? k)
                = k $ floor k
                &>= k 0
                &< k $ count $ [] 1 2
              &list:assoc ([] 1 2) k 3
              [] 1 2
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Dynamic
            :return $ :: 'List 'Number
          :tests $ []
            %{} 'TestEntry (:name |checked-index)
              :code $ quote $ do
                assert= ([] 1 3) (compound 1)
                assert= ([] 1 2) (compound |bad)
                assert= ([] 1 2) (compound -1)
                assert= ([] 1 2) (compound 1.5)
                assert= ([] 1 2) (compound 2)
              :tags $ #{} :short-circuit-proof :unit
            %{} 'TestEntry (:name |single-evaluation)
              :code $ quote $ let
                  calls $ atom 0
                assert= false $ and false $ do
                  reset! calls $ inc $ deref calls
                  , true
                assert= 0 $ deref calls
                assert= 7 $ and true $ do
                  reset! calls $ inc $ deref calls
                  , 7
                assert= 1 $ deref calls
                assert= 7 $ or 7 $ do (reset! calls 99) false
                assert= 1 $ deref calls
              :tags $ #{} :short-circuit-proof :unit
        'false-path $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn false-path (k)
            if
              if (number? k) false true
              , 0 $ &+ k 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |false-branch-proof)
            :code $ quote $ do
              assert= 2 $ false-path 1
              assert= 0 $ false-path |bad
            :tags $ #{} :short-circuit-proof :unit
        'lexical-guard $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lexical-guard (k)
            if
              let
                  valid $ number? k
                if valid true false
              &+ k 1
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |bound-predicate)
            :code $ quote $ do
              assert= 2 $ lexical-guard 1
              assert= 0 $ lexical-guard |bad
            :tags $ #{} :short-circuit-proof :unit
        'nested $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nested (k)
            if (number? k)
              if
                and
                  = k $ floor k
                  &>= k 0
                  &< k $ count $ [] 1 2
                &list:assoc ([] 1 2) k 3
                [] 1 2
              [] 1 2
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Dynamic
            :return $ :: 'List 'Number
          :tests $ [] $ %{} 'TestEntry (:name |checked-index)
            :code $ quote $ do
              assert= ([] 1 3) (nested 1)
              assert= ([] 1 2) (nested |bad)
              assert= ([] 1 2) (nested -1)
              assert= ([] 1 2) (nested 1.5)
              assert= ([] 1 2) (nested 2)
            :tags $ #{} :short-circuit-proof :unit
        'two-values $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn two-values (x y)
            if
              and (number? x) (number? y) (&< x y)
              &+ x y
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |both-operands)
            :code $ quote $ do
              assert= 5 $ two-values 2 3
              assert= 0 $ two-values 2 |bad
              assert= 0 $ two-values |bad 3
            :tags $ #{} :short-circuit-proof :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.short-circuit
    'app.values $ %{} 'FileEntry
      :defs $ {}
        'ClientProjection $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ClientProjection (:count 'Number) (:color 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'LoginState $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct LoginState (:username 'String) (:password 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'PatchCandidate $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct PatchCandidate
            :raw $ :: 'Map 'Tag 'Dynamic
            :typed 'app.values/ClientProjection
          :examples $ []
          :schema $ :: 'StructDef
        'initial-state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def initial-state (LoginState :username | :password |)
          :examples $ []
          :schema $ :: 'app.values/LoginState
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.values
