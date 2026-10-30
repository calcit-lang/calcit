
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |legacy-optional-advisory
  :entries $ {} $ :default
    {}
      :description "|Legacy Optional<T> is rejected while loading the Snapshot (E_LEGACY_OPTIONAL_TYPE); the error lists every position and leaves the Option forms out."
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
          :doc "|Removed-syntax fixture: still spells Optional to pin the E_LEGACY_OPTIONAL_TYPE load error (0.33.0)."
          :code $ quote $ defstruct Box
            :value $ quote $ Optional Number
            :label 'String
          :examples $ []
          :schema $ quote StructDef
        'Choice $ %{} 'CodeEntry
          :doc "|Removed-syntax fixture: still spells Optional to pin the E_LEGACY_OPTIONAL_TYPE load error (0.33.0)."
          :code $ quote $ defenum Choice
            :maybe $ quote $ Optional String
            :plain 'Number
          :examples $ []
          :schema $ quote EnumDef
        'OptionBox $ %{} 'CodeEntry (:doc "|The Option form of Box is not listed.")
          :code $ quote $ defstruct OptionBox
            :value $ quote $ Option Number
          :examples $ []
          :schema $ quote StructDef
        'fallback $ %{} 'CodeEntry
          :doc "|Removed-syntax fixture: still spells Optional to pin the E_LEGACY_OPTIONAL_TYPE load error (0.33.0)."
          :code $ quote $ def fallback nil
          :examples $ []
          :schema $ quote $ Optional Number
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
          :schema $ quote $ Fn (-> Unit)
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'use-assert-type $ %{} 'CodeEntry
          :doc "|Removed-syntax fixture: still spells Optional to pin the E_LEGACY_OPTIONAL_TYPE load error (0.33.0)."
          :code $ quote $ defn use-assert-type (x)
            let
                y x
              assert-type y $ quote $ Optional Number
              , &unit
          :examples $ []
          :schema $ quote $ Fn Number (-> Unit)
        'use-decoder $ %{} 'CodeEntry
          :doc "|Removed-syntax fixture: still spells Optional to pin the E_LEGACY_OPTIONAL_TYPE load error (0.33.0)."
          :code $ quote $ defn use-decoder (text)
            let
                decoded $ parse-cirru-edn-as text $ :: 'Optional 'Number
              , &unit
          :examples $ []
          :schema $ quote $ Fn String (-> Unit)
        'use-hint-fn $ %{} 'CodeEntry
          :doc "|Removed-syntax fixture: still spells Optional to pin the E_LEGACY_OPTIONAL_TYPE load error (0.33.0)."
          :code $ quote $ defn use-hint-fn (x)
            let
                pick $ fn (value)
                  hint-fn $ Fn (Optional Number) (-> Bool)
                  nil? value
              pick x
              , &unit
          :examples $ []
          :schema $ quote $ Fn Number (-> Unit)
        'use-option-hint $ %{} 'CodeEntry (:doc "|A local hint-fn with Option is not listed.")
          :code $ quote $ defn use-option-hint (x)
            let
                pick $ fn (value)
                  hint-fn $ Fn (Option Number) (-> Bool)
                  .some? value
              pick $ Option :some x
              , &unit
          :examples $ []
          :schema $ quote $ Fn Number (-> Unit)
      :ns $ %{} 'NsEntry (:doc "|Legacy Optional advisory fixture.")
        :code $ quote $ ns legacy-optional-advisory.main
