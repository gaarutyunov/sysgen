// SysML v2 example spec — used for integration tests

package ExampleSystem {

    part def Vehicle {
        attribute mass : Real;
        attribute maxSpeed : Real;
    }

    part def Engine {
        attribute power : Real;
        attribute fuelType : String;
    }

    part vehicle : Vehicle {
        part engine : Engine;
    }

}
