
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |fix-command)
  :entries $ {}
    :default $ {} (:description "|Compiler-guided source fix fixture.") (:init-fn 'fix-command.main/main!) (:mode :native) (:reload-fn 'fix-command.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'fix-command.main $ %{} 'FileEntry
      :defs $ {}
        '*fix-person-calls $ %{} 'CodeEntry (:doc |)
          :code $ quote (defatom *fix-person-calls 0)
          :examples $ []
          :schema $ :: 'Dynamic
        'FixPerson $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct FixPerson (:name 'String) (:age 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'FixPersonChoice $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defenum FixPersonChoice (:person 'fix-command.main/FixPerson) (:none)
          :examples $ []
          :schema $ :: 'EnumDef
        'ambiguous $ %{} 'CodeEntry (:doc "|Removed predicate needs a semantic choice.")
          :code $ quote
            defn ambiguous (value) (tuple? value)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
        'defaulted-struct-field $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn defaulted-struct-field (person) (:name person)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'fix-command.main/FixPerson
          :tests $ []
            %{} 'TestEntry (:name |keeps-business-default-path)
              :code $ quote
                assert= |Ada $ defaulted-struct-field (FixPerson :name |Ada :age 1)
              :tags $ #{} :migration
        'dynamic-struct-field $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn dynamic-struct-field (value) (get value :name)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |keeps-dynamic-lookup)
              :code $ quote
                assert= (%some |Ada)
                  dynamic-struct-field $ {} (:name |Ada)
              :tags $ #{} :migration
        'fixable $ %{} 'CodeEntry (:doc "|Exact removed API call.")
          :code $ quote
            defn fixable (value) (tuple-enum value)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Dynamic
              :return $ :: 'calcit.core/Option 'EnumDef
          :tests $ []
            %{} 'TestEntry (:name |returns-enum-definition)
              :code $ quote
                assert= (%some Option)
                  fixable $ %some 1
              :tags $ #{} :migration
        'macro-origin-struct-field $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn macro-origin-struct-field (person) (:name person)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'fix-command.main/FixPerson
          :tests $ []
            %{} 'TestEntry (:name |keeps-ambiguous-macro-origin)
              :code $ quote
                assert= |Ada $ macro-origin-struct-field (FixPerson :name |Ada :age 1)
              :tags $ #{} :migration
        'main! $ %{} 'CodeEntry (:doc |Entry.)
          :code $ quote
            defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'make-fix-person $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn make-fix-person () (swap! *fix-person-calls inc) (FixPerson :name |Ada :age 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'fix-command.main/FixPerson)
              :args $ []
        'option-struct-field $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn option-struct-field (person) (get person :name)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] (:: 'calcit.core/Option 'fix-command.main/FixPerson)
          :tests $ []
            %{} 'TestEntry (:name |keeps-option-receiver)
              :code $ quote
                assert= (%none)
                  option-struct-field $ %some (FixPerson :name |Ada :age 1)
              :tags $ #{} :migration
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'required-struct-complex $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn required-struct-complex () $ :name (make-fix-person)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ []
          :tests $ []
            %{} 'TestEntry (:name |evaluates-complex-receiver-once)
              :code $ quote
                do (reset! *fix-person-calls 0)
                  assert= |Ada $ required-struct-complex
                  assert= 1 @*fix-person-calls
              :tags $ #{} :migration
        'required-struct-field $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn required-struct-field (person) (:name person)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'fix-command.main/FixPerson
          :tests $ []
            %{} 'TestEntry (:name |migrates-declared-field)
              :code $ quote
                assert= |Ada $ required-struct-field (FixPerson :name |Ada :age 1)
              :tags $ #{} :migration
        'runtime-struct-field $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn runtime-struct-field (person field) (get person field)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Tag
          :tests $ []
            %{} 'TestEntry (:name |keeps-runtime-key)
              :code $ quote
                assert= (%some |Ada)
                  runtime-struct-field (FixPerson :name |Ada :age 1) :name
              :tags $ #{} :migration
        'shadowed $ %{} 'CodeEntry (:doc "|A local binding that shares the removed core API name.")
          :code $ quote
            defn shadowed (tuple-enum value) (tuple-enum value)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
                :: 'Fn $ {} (:return 'Dynamic)
                  :args $ [] 'Dynamic
                , 'Dynamic
        'shadowed-struct-get $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn shadowed-struct-get (get person) (get person :name)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ []
                :: 'Fn $ {} (:return 'String)
                  :args $ [] 'fix-command.main/FixPerson 'Tag
                , 'fix-command.main/FixPerson
          :tests $ []
            %{} 'TestEntry (:name |keeps-local-get-shadow)
              :code $ quote
                assert= |shadow $ shadowed-struct-get
                  fn (person field) |shadow
                  FixPerson :name |Ada :age 1
              :tags $ #{} :migration
        'union-struct-field $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn union-struct-field (person) (get person :name)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'fix-command.main/FixPersonChoice
          :tests $ []
            %{} 'TestEntry (:name |keeps-enum-union-receiver)
              :code $ quote
                assert= (%none)
                  union-struct-field $ FixPersonChoice :person (FixPerson :name |Ada :age 1)
              :tags $ #{} :migration
        'unknown-struct-field $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn unknown-struct-field (person) (get person :missing)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |keeps-unknown-field-for-review)
              :code $ quote
                assert= (%none)
                  unknown-struct-field $ FixPerson :name |Ada :age 1
              :tags $ #{} :migration
      :ns $ %{} 'NsEntry (:doc "|Fix command fixture.")
        :code $ quote (ns fix-command.main)
