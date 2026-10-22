
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |legacy-optional-advisory
  :entries $ {} $ :default
    {}
      :description "|Legacy Optional<T> outside public function schemas is only noted (W_LEGACY_OPTIONAL_TYPE); the strict check still passes."
      :init-fn 'legacy-optional-advisory.main/main!
      :mode :native
      :reload-fn 'legacy-optional-advisory.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'legacy-optional-advisory.main
    %{} 'FileEntry
      :defs $ {}
        'Box $ %{} 'CodeEntry
          :doc "|Legacy-contract fixture: a struct field still spelled Optional; retire with Optional (0.32.0)."
          :code $ quote $ defstruct Box
            :value $ :: 'Optional 'Number
            :label 'String
          :examples $ []
          :schema $ :: 'StructDef
        'Choice $ %{} 'CodeEntry
          :doc "|Legacy-contract fixture: an enum payload still spelled Optional; retire with Optional (0.32.0)."
          :code $ quote $ defenum Choice
            :maybe $ :: 'Optional 'String
            :plain 'Number
          :examples $ []
          :schema $ :: 'EnumDef
        'OptionBox $ %{} 'CodeEntry (:doc "|The Option form of Box is not noted.")
          :code $ quote $ defstruct OptionBox
            :value $ :: 'Option 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'fallback $ %{} 'CodeEntry
          :doc "|Legacy-contract fixture: a value schema still spelled Optional; retire with Optional (0.32.0)."
          :code $ quote $ def fallback nil
          :examples $ []
          :schema $ :: 'Optional 'Number
        'main! $ %{} 'CodeEntry (:doc "|Reaches every position.")
          :code $ quote $ defn main! ()
            let
                box $ Box :value fallback :label |a
                option-box $ OptionBox :value $ Option :none
                choice $ Choice :maybe nil
              use-hint-fn 1
              use-assert-type 2
              use-option-hint 3
              use-decoder "|do 1"
              , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'use-assert-type $ %{} 'CodeEntry
          :doc "|Legacy-contract fixture: an assert-type target still spelled Optional; retire with Optional (0.32.0)."
          :code $ quote $ defn use-assert-type (x)
            let
                y x
              assert-type y $ :: 'Optional 'Number
              , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'Number
        'use-decoder $ %{} 'CodeEntry
          :doc "|Legacy-contract fixture: a decoder target still spelled Optional; retire with Optional (0.32.0)."
          :code $ quote $ defn use-decoder (text)
            let
                decoded $ parse-cirru-edn-as text $ :: 'Optional 'Number
              , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'String
        'use-hint-fn $ %{} 'CodeEntry
          :doc "|Legacy-contract fixture: a local hint-fn still spelled Optional; retire with Optional (0.32.0)."
          :code $ quote $ defn use-hint-fn (x)
            let
                pick $ fn (value)
                  hint-fn $ {}
                    :args $ [] $ :: 'Optional 'Number
                    :return 'Bool
                  nil? value
              pick x
              , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'Number
        'use-option-hint $ %{} 'CodeEntry (:doc "|A local hint-fn with Option is not noted.")
          :code $ quote $ defn use-option-hint (x)
            let
                pick $ fn (value)
                  hint-fn $ {}
                    :args $ [] $ :: 'Option 'Number
                    :return 'Bool
                  .some? value
              pick $ Option :some x
              , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'Number
      :ns $ %{} 'NsEntry (:doc "|Legacy Optional advisory fixture.")
        :code $ quote $ ns legacy-optional-advisory.main
