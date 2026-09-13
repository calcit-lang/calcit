
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-optimize
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'test-optimize.main/main!
      :mode :native
      :reload-fn 'test-optimize.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'test-optimize.main
    %{} 'FileEntry
      :defs $ {}
        'LocalPerson0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct LocalPerson0 (:name 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'Person $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Person (impl-traits Person0 ShowImpl)
          :examples $ []
          :schema $ :: 'Dynamic
        'Person0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Person0 (:name 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'ShowImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl ShowImpl ShowTrait
            .show $ fn (self)
              str "|Person: " $ &struct:get self :name
          :examples $ []
          :schema $ :: 'Impl
        'ShowTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait ShowTrait (.show :fn)
          :examples $ []
          :schema $ :: 'Trait
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                p $ %{} Person $ :name |Jim
              println "|--- direct call ---"
              println $ p .show
              let
                  p2 p
                assert-traits p2 ShowTrait
                println "|--- assert-traits ShowTrait ---"
                println $ p2 .show
              let
                  p3 p
                assert-type p3 Person
                println "|--- assert-type Person ---"
                println $ p3 .show
              let
                  p4 p
                assert-type p4 Person
                assert-traits p4 ShowTrait
                println "|--- assert-type Person + assert-traits ShowTrait ---"
                println $ p4 .show
              let
                LocalPerson $ impl-traits LocalPerson0 ShowImpl $ lp
                  %{} LocalPerson $ :name |Local
                println "|--- local struct (runtime impl) ---"
                assert-traits lp ShowTrait
                println $ lp .show
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-optimize.main (:require)
