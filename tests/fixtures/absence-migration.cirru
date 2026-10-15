
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'f $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f (n)
            if (= n 0) 0 $ + 1 $ f (- n 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (println |reload)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
    'app.migration $ %{} 'FileEntry
      :defs $ {}
        'TraversalBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct TraversalBox (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'active-error $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn active-error ()
            let
                calls $ ref 0
                result $ try
                  either
                    &map:get (&{} :k true) :missing
                    do (reset! calls 1) (raise |expected-fallback-error) false
                  fn (error) true
              assert= 1 $ deref calls
              , result
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/active-error)
            :tags $ #{} :absence-migration
        'closure-identity $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn closure-identity ()
            let
                fallback $ fn () 7
                result $ either
                  &map:get (&{} :k fallback) :missing
                  , fallback
              identical? fallback result
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/closure-identity)
            :tags $ #{} :absence-migration
        'enum-probe $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn enum-probe ()
            nil? $ &enum:definition $ Option :some 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= false (app.migration/enum-probe)
            :tags $ #{} :absence-migration
        'env-probe $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn env-probe ()
            nil? $ &get-env |CALCIT_ABSENCE_MIGRATION_UNSET_2080
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/env-probe)
            :tags $ #{} :absence-migration
        'fallback-either $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn fallback-either ()
            let
                counter $ ref 0
                result $ either
                  &map:get (&{} :k nil) :k
                  do
                    reset! counter $ inc $ deref counter
                    , 9
                  do
                    reset! counter $ + 100 $ deref counter
                    , 100
              assert= 1 $ deref counter
              assert= 9 result
              , true
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |lazy-once-only)
            :code $ quote $ assert= true (app.migration/fallback-either)
            :tags $ #{} :absence-migration
        'impl-probe $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn impl-probe ()
            nil? $ &impl:origin $ &impl::new :PlainImpl
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/impl-probe)
            :tags $ #{} :absence-migration
        'list-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn list-first ()
            nil? $ &list:first $ [] nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/list-first)
            :tags $ #{} :absence-migration
        'list-last $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn list-last ()
            nil? $ &list:last $ [] 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= false (app.migration/list-last)
            :tags $ #{} :absence-migration
        'map-destruct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-destruct ()
            nil? $ &map:destruct $ &{}
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/map-destruct)
            :tags $ #{} :absence-migration
        'map-destruct-present $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-destruct-present ()
            non-nil? $ &map:destruct $ &{} :k 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/map-destruct-present)
            :tags $ #{} :absence-migration
        'map-destruct-values $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-destruct-values ()
            let
                result $ either
                  &map:destruct $ &{} :k 1
                  [] :fallback
              assert=
                [] :k 1 $ &{}
                , result
              , true
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-payload)
            :code $ quote $ assert= true (app.migration/map-destruct-values)
            :tags $ #{} :absence-migration
        'map-either $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-either ()
            let
                counter $ ref 0
                result $ either
                  &map:get
                    do
                      assert= 0 $ deref counter
                      reset! counter $ inc $ deref counter
                      &{} :k 7
                    do
                      assert= 1 $ deref counter
                      reset! counter $ inc $ deref counter
                      , :k
                  do (raise |fallback-must-remain-lazy) 9
              assert= 2 $ deref counter
              , result
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |lazy-once-only)
            :code $ quote $ assert= 7 (app.migration/map-either)
            :tags $ #{} :absence-migration
        'map-empty-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-empty-text ()
            nil? $ &map:get (&{} :k |) :k
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= false (app.migration/map-empty-text)
            :tags $ #{} :absence-migration
        'map-false $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-false ()
            nil? $ &map:get (&{} :k false) :k
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= false (app.migration/map-false)
            :tags $ #{} :absence-migration
        'map-missing $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-missing ()
            nil? $ &map:get (&{} :k 7) :missing
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/map-missing)
            :tags $ #{} :absence-migration
        'map-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-nil ()
            nil? $ &map:get (&{} :k nil) :k
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |legacy-value)
              :code $ quote $ assert= true (app.migration/map-nil)
              :tags $ #{} :absence-migration
            %{} 'TestEntry (:name |attached-source)
              :code $ quote $ assert= true
                nil? $ &map:get (&{} :k nil) :k
              :tags $ #{} :absence-migration
        'map-zero $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-zero ()
            nil? $ &map:get (&{} :k 0) :k
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= false (app.migration/map-zero)
            :tags $ #{} :absence-migration
        'nested-lookup $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nested-lookup ()
            nil? $ &get-in (&{} :k nil) ([] :k)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/nested-lookup)
            :tags $ #{} :absence-migration
        'parse-either $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn parse-either ()
            either (&parse-float |oops) 9
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |lazy-once-only)
            :code $ quote $ assert= 9 (app.migration/parse-either)
            :tags $ #{} :absence-migration
        'parse-failed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn parse-failed ()
            nil? $ &parse-float |oops
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/parse-failed)
            :tags $ #{} :absence-migration
        'parse-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn parse-number ()
            non-nil? $ &parse-float |1.5
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/parse-number)
            :tags $ #{} :absence-migration
        'public-struct-boundary $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn public-struct-boundary (base)
            try
              do
                get-in base $ [] :value
                , |unexpected-success
              fn (message)
                hint-fn $ {}
                  :args $ [] 'String
                  :return 'String
                , message
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |public-struct-error)
            :code $ quote $ assert= "|get-in does not traverse Struct fields; use (:field value) so the checker can enforce the declared type"
              app.migration/public-struct-boundary $ TraversalBox :value 3
            :tags $ #{} :absence-migration
        'set-destruct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn set-destruct ()
            non-nil? $ &set:destruct $ #{} 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/set-destruct)
            :tags $ #{} :absence-migration
        'set-destruct-values $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn set-destruct-values ()
            let
                result $ either
                  &set:destruct $ #{} 1
                  [] :fallback
              assert=
                [] 1 $ #{}
                , result
              , true
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-payload)
            :code $ quote $ assert= true (app.migration/set-destruct-values)
            :tags $ #{} :absence-migration
        'set-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn set-empty ()
            nil? $ &set:destruct $ #{}
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/set-empty)
            :tags $ #{} :absence-migration
        'string-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn string-first ()
            nil? $ &str:first |
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/string-first)
            :tags $ #{} :absence-migration
        'string-nth $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn string-nth ()
            non-nil? $ &str:nth "|🌱树" 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/string-nth)
            :tags $ #{} :absence-migration
        'string-outside $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn string-outside ()
            nil? $ &str:nth |abc 5
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/string-outside)
            :tags $ #{} :absence-migration
        'struct-probe $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn struct-probe ()
            let
                Person $ defstruct Person $ :name 'String
              nil? $ &struct:definition $ Person :name |Ada
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= false (app.migration/struct-probe)
            :tags $ #{} :absence-migration
        'traversal-empty-scalar $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn traversal-empty-scalar ()
            non-nil? $ &get-in false $ []
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/traversal-empty-scalar)
            :tags $ #{} :absence-migration
        'traversal-list-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn traversal-list-map ()
            nil? $ &get-in
              [] $ &{} :k nil
              [] 0 :k
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/traversal-list-map)
            :tags $ #{} :absence-migration
        'traversal-map-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn traversal-map-list ()
            non-nil? $ &get-in
              &{} :k $ [] $ &{} :value false
              [] :k 0 :value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/traversal-map-list)
            :tags $ #{} :absence-migration
        'traversal-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn traversal-nil ()
            nil? $ &get-in nil $ [] :k :value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/traversal-nil)
            :tags $ #{} :absence-migration
        'traversal-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn traversal-string ()
            non-nil? $ &get-in "|😀" $ [] 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/traversal-string)
            :tags $ #{} :absence-migration
        'traversal-struct-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn traversal-struct-empty ()
            non-nil? $ &get-in (TraversalBox :value 3) ([])
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/traversal-struct-empty)
            :tags $ #{} :absence-migration
        'traversal-struct-terminal $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn traversal-struct-terminal ()
            non-nil? $ &get-in
              &{} :box $ TraversalBox :value 3
              [] :box
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |legacy-value)
            :code $ quote $ assert= true (app.migration/traversal-struct-terminal)
            :tags $ #{} :absence-migration
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.migration
