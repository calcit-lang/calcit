
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-enum
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-enum.main/main!) (:mode :native) (:reload-fn 'test-enum.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'test-enum.main $ %{} 'FileEntry
      :defs $ {}
        'ContentNode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ContentNode (:wrong 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'Duo $ %{} 'CodeEntry (:doc "|Generic enum with 2 type variables")
          :code $ quote $ defenum Duo ('T 'U) (:pair 'T 'U) (:swapped 'U 'T)
          :examples $ []
          :schema $ :: 'EnumDef
        'MapLiteralChoice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum MapLiteralChoice
            {} (:some 'String) (:none)
          :examples $ []
          :schema $ :: 'EnumDef
          :tests $ [] $ %{} 'TestEntry (:name |map-literal-variants)
            :code $ quote $ match (MapLiteralChoice :some |ok)
              (:some value) (assert= |ok value)
              (:none) (raise |expected-some)
        'Maybe1 $ %{} 'CodeEntry (:doc "|Generic enum with 1 type variable")
          :code $ quote $ defenum Maybe1 ('T) (:some 'T) (:none)
          :examples $ []
          :schema $ :: 'EnumDef
        'Payload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Payload (:wrong 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'Result0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Result0 (:err 'String) (:ok)
          :examples $ []
          :schema $ :: 'EnumDef
        'ResultImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl ResultImpl ResultTrait
            .dummy $ fn $ _x
          :examples $ []
          :schema $ :: 'Impl
        'ResultTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait ResultTrait
            .dummy $ :: :fn $ {}
              :generics $ [] 'T
              :args $ [] 'T
              :return 'Unit
          :examples $ []
          :schema $ :: 'Trait
        'ShownBox $ %{} 'CodeEntry (:doc "|Generic struct with where-bound on payload type")
          :code $ quote $ defstruct ShownBox ('T)
            {} $ 'T Debug
            :value 'T
          :examples $ []
          :schema $ :: 'StructDef
        'ShownMaybe $ %{} 'CodeEntry (:doc "|Generic enum with where-bound on payload type")
          :code $ quote $ defenum ShownMaybe ('T)
            {} $ 'T Debug
            :some 'T
            :none
          :examples $ []
          :schema $ :: 'EnumDef
        'check-result-type $ %{} 'CodeEntry (:doc "|Check if value has enum origin")
          :code $ quote $ defn check-result-type (r)
            option:some? $ enum-definition r
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'test-enum.main/Result0
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            do (println "|Testing enum runtime validation...") (test-enum-creation) (test-generic-enum-creation) (test-generic-enum-where-bounds) (test-where-bound-definitions) (test-match-validation) (test-anonymous-enum-to-named) (test-match) (println "|All tests passed!")
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'read-caller-payload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-caller-payload (value)
            :wrong $ unwrap-recursive-envelope value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'test-enum.recursive/Envelope 'test-enum.main/Payload
          :tests $ [] $ %{} 'TestEntry (:name |generic-argument-keeps-caller-namespace)
            :code $ quote $ assert= 7
              read-caller-payload $ recursive/Envelope :item $ Payload :wrong 7
            :tags $ #{} :enum :nominal-namespace :unit
        'read-declared-payload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-declared-payload (value)
            match value $
              :record payload
              :value payload
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'test-enum.recursive/RecordEnvelope
          :tests $ [] $ %{} 'TestEntry (:name |declared-payload-does-not-bind-caller-shadow)
            :code $ quote $ assert= |Ada
              read-declared-payload $ recursive/RecordEnvelope :record $ recursive/Payload :value |Ada
            :tags $ #{} :enum :nominal-namespace :unit
        'read-local-shadow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-local-shadow (value)
            let
                Payload test-enum.main/Payload
              match value $
                :record payload
                :value payload
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'test-enum.recursive/RecordEnvelope
          :tests $ [] $ %{} 'TestEntry (:name |qualified-payload-ignores-local-shadow)
            :code $ quote $ assert= |Ada
              read-local-shadow $ recursive/RecordEnvelope :record $ recursive/Payload :value |Ada
            :tags $ #{} :enum :local-type-shadow :unit
        'read-local-type $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-local-type ()
            let
                Payload recursive/Payload
                read $ fn (record)
                  hint-fn $ {}
                    :args $ [] 'Payload
                    :return 'String
                  :value record
                record $ Payload :value |Ada
              assert-type record 'Payload
              read record
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |unqualified-local-type-remains-lexical)
            :code $ quote $ assert= |Ada (read-local-type)
            :tags $ #{} :enum :local-type-shadow :unit
        'read-shadowed-local-type $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-shadowed-local-type ()
            let
                Payload recursive/Payload
                read $ fn (Payload)
                  hint-fn $ {}
                    :args $ [] 'String
                    :return 'Payload
                  let
                      record $ recursive/Payload :value Payload
                    assert-type record 'Payload
                    assert-type record Payload
                    , record
              :value $ read |Ada
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |parameter-does-not-rebind-outer-type-alias)
            :code $ quote $ assert= |Ada (read-shadowed-local-type)
            :tags $ #{} :enum :local-type-shadow :unit
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (println |Reloaded)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'render-short-recursive $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn render-short-recursive (node)
            match node
              (:text text) text
              (:pair left right)
                str (render-short-recursive left) (render-short-recursive right)
              (:list children)
                foldl children | $ fn (acc child)
                  str acc $ render-short-recursive child
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'test-enum.recursive/ContentNode
          :tests $ [] $ %{} 'TestEntry (:name |recursive-short-names-in-declaration-namespace)
            :code $ quote $ do
              assert= |ab $ render-short-recursive $ recursive/ContentNode :pair (recursive/ContentNode :text |a) (recursive/ContentNode :text |b)
              assert= |ab $ render-short-recursive $ recursive/ContentNode :list
                [] (recursive/ContentNode :text |a) (recursive/ContentNode :text |b)
              assert= | $ render-short-recursive $ recursive/ContentNode :list ([])
            :tags $ #{} :enum :nominal-namespace :unit
        'takes-result $ %{} 'CodeEntry (:doc "|Function accepting Result0 enum type")
          :code $ quote $ defn takes-result (r)
            match r
              (:ok) :ok
              (:err msg) msg
              _ :unknown
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'test-enum.main/Result0
        'test-anonymous-enum-to-named $ %{} 'CodeEntry (:doc "|Test automatic anonymous-enum-to-named rewrite")
          :code $ quote $ defn test-anonymous-enum-to-named ()
            do (println "|Testing anonymous-enum-to-named rewrite...")
              ; Untyped anonymous enum :: :ok gets rewritten to %:: Result0 :ok
              assert= :ok $ takes-result $ :: :ok
              ; Untyped tuple with payload
              assert= |error-msg $ takes-result $ :: :err |error-msg
              ; Verify the rewritten value has enum origin
              assert= true $ check-result-type $ :: :ok
              println "|✓ Tuple-to-enum rewrite passed"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-enum-creation $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-enum-creation ()
            do (println "|Testing enum tuple creation...")
              let
                  valid-ok $ %:: Result0 :ok
                  Result1 $ impl-traits Result0 ResultImpl
                assert= :ok $ &enum:nth valid-ok 0
                let
                    ok-impl $ %:: Result1 :ok
                  assert= true $ any? (&enum:impls ok-impl)
                    fn (impl)
                      includes? (str impl) |ResultTrait
                  assert= "|(%:: 'Result0 :ok)" $ str ok-impl
              let
                  valid-err $ %:: Result0 :err |error-msg
                assert= :err $ &enum:nth valid-err 0
                assert= true $ enum? valid-err
              ; Test invalid tag $ should fail - uncomment to see error
              ; let
                (invalid (%:: Result0 :invalid))
                raise "|Should have failed with invalid tag"
              ; Test wrong arity $ should fail - uncomment to see error
              ; let
                (wrong-arity (%:: Result0 :ok |extra))
                raise "|Should have failed with wrong arity"
              println "|✓ Enum creation validation passed"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-generic-enum-creation $ %{} 'CodeEntry
          :doc "|Exercise defenum generic variables in runtime creation and matching"
          :code $ quote $ defn test-generic-enum-creation ()
            do (println "|Testing generic enum creation...")
              let
                  some-num $ %:: Maybe1 :some 1
                  none-value $ %:: Maybe1 :none
                  pair-value $ %:: Duo :pair 1 |hi
                  swapped-value $ %:: Duo :swapped |hi 1
                assert-type some-num $ :: 'Maybe1 'Number
                assert-type none-value $ :: 'Maybe1 'Dynamic
                assert-type pair-value $ :: 'Duo 'Number 'String
                assert-type swapped-value $ :: 'Duo 'Number 'String
                assert= (Option :some 1)
                  unwrap-maybe $ %:: Maybe1 :some 1
                assert= (Option :none)
                  unwrap-maybe $ %:: Maybe1 :none
                assert= :pair $ &enum:nth pair-value 0
                assert= 1 $ &enum:nth pair-value 1
                assert= |hi $ &enum:nth pair-value 2
              println "|✓ Generic enum creation passed"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-generic-enum-where-bounds $ %{} 'CodeEntry
          :doc "|Exercise where-bounds with generic enum payloads"
          :code $ quote $ defn test-generic-enum-where-bounds ()
            let
                render-maybe $ fn (v)
                  hint-fn $ {}
                    :generics $ [] 'T
                    :where $ {} $ 'T Debug
                    :args $ [] $ :: 'Maybe1 'T
                    :return 'String
                  match v
                    (:none) |none
                    (:some item) (item .debug)
                render-duo $ fn (v)
                  hint-fn $ {}
                    :generics $ [] 'T 'U
                    :where $ {} ('T Debug) ('U Debug)
                    :args $ [] $ :: 'Duo 'T 'U
                    :return 'String
                  match v
                    (:pair left right)
                      str-spaced |pair (left .debug) (right .debug)
                    (:swapped right left)
                      str-spaced |swapped (right .debug) (left .debug)
              println "|Testing generic enum where-bounds..."
              assert= |1 $ render-maybe $ %:: Maybe1 :some 1
              assert= |none $ render-maybe $ assert-type (Maybe1 :none) (:: 'Maybe1 'Number)
              assert= (str-spaced |pair |1 |hi)
                render-duo $ %:: Duo :pair 1 |hi
              assert= (str-spaced |swapped |hi |1)
                render-duo $ %:: Duo :swapped |hi 1
              println "|✓ Generic enum where-bounds passed"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-match ()
            let
                result-ok $ %:: Result0 :ok
                v $ match result-ok
                  (:ok) :matched-ok
                  (:err msg) msg
              assert= :matched-ok v
            let
                result-err $ %:: Result0 :err |some-error
                v $ match result-err
                  (:ok) :matched-ok
                  (:err msg) msg
              assert= |some-error v
            ; Test exhaustive match with wildcard
            let
                result-ok $ %:: Result0 :ok
                v $ match result-ok
                  (:ok) :ok-branch
                  _ :default-branch
              assert= :ok-branch v
            println "|✓ match syntax passed"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-match-validation $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-match-validation ()
            do (println "|Testing match runtime validation...")
              let
                  result $ %:: Result0 :ok
                  v $ match result
                    (:ok) :ok
                    _ :unknown
                assert= :ok v
              println "|✓ Match validation passed"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-where-bound-definitions $ %{} 'CodeEntry
          :doc "|Exercise defstruct/defenum where-map syntax on generic data types"
          :code $ quote $ defn test-where-bound-definitions ()
            do (println "|Testing data definition where-bounds...")
              let
                  box $ %{} ShownBox $ :value 1
                let
                    some-value $ %:: ShownMaybe :some 1
                  let
                      none-value $ %:: ShownMaybe :none
                    assert-type box $ :: 'ShownBox 'Number
                    assert-type some-value $ :: 'ShownMaybe 'Number
                    assert= |1 $ match some-value
                      (:some item) (item .debug)
                      (:none) |none
                    assert= |none $ match none-value
                      (:some item) (item .debug)
                      (:none) |none
              println "|✓ Data definition where-bounds passed"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'unwrap-maybe $ %{} 'CodeEntry (:doc "|Convert Maybe1<T> into nominal Option<T>.")
          :code $ quote $ defn unwrap-maybe (v)
            match v
              (:none) (Option :none)
              (:some item) (Option :some item)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'test-enum.main/Maybe1 'T
            :generics $ [] 'T
            :return $ :: 'calcit.core/Option 'T
        'unwrap-recursive-envelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unwrap-recursive-envelope (value)
            match value $
              :item payload
              , payload
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'T)
            :args $ [] $ :: 'test-enum.recursive/Envelope 'T
            :generics $ [] 'T
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-enum.main
          :require $ test-enum.recursive :as recursive
    'test-enum.recursive $ %{} 'FileEntry
      :defs $ {}
        'ContentNode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum ContentNode (:text 'String) (:pair 'ContentNode 'ContentNode)
            :list $ :: 'List 'ContentNode
          :examples $ []
          :schema $ :: 'EnumDef
        'Envelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Envelope ('T) (:item 'T)
          :examples $ []
          :schema $ :: 'EnumDef
        'Payload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Payload (:value 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'RecordEnvelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum RecordEnvelope (:record 'Payload)
          :examples $ []
          :schema $ :: 'EnumDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-enum.recursive
