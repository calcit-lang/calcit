
{}
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'CssValue $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion CssValue 'String 'Number
          :examples $ []
        'Geo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Geo 'Point 'Shape 'String
          :examples $ []
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x 'Number) (:y 'Number)
          :examples $ []
        'Shape $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Shape (:circle 'Number) (:square 'Number)
          :examples $ []
        'f $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f (v) (show-it v)
          :examples $ []
          :schema $ quote $ Fn CssValue (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println (f |a)
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'show-it $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-it (x) (to-lispy-string x)
          :examples $ []
          :schema $ quote $ Fn T (-> String) (:generics T) (:where (T Hash))
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
