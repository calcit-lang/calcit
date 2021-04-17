
{} $ :changed
  {} $ |app.main
    {} $ :changed-defs
      {} $ |main!
        quote $ defn main! () (echo "\"demo")
          echo $ &+ 2 2
          echo "\"f1" $ f1
          echo-values 1 "\"1" :a $ [] 1 2
          echo $ &{} :a 1 :b 2
          echo $ #{} 1 2 3 |four
          lib/f2
          f3 "\"arg of 3"
